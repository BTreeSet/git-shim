//! Robust resolution of the current user's *Local AppData* directory.
//!
//! ## Why this isn't just `std::env::var_os("LOCALAPPDATA")`
//!
//! In 99.99% of Windows process contexts the `LOCALAPPDATA` environment
//! variable is set by the logon session, and the env lookup is the cheapest,
//! cleanest answer. But it can be missing when a parent process scrubs the
//! environment — e.g. some sandboxed launchers, some `LocalSystem` services,
//! and the integration test in `tests/canonical_invocation.rs` (which strips
//! the variable on purpose to exercise the failure path).
//!
//! For those cases we fall back to [`SHGetKnownFolderPath`] with
//! `FOLDERID_LocalAppData`. This is the API Windows itself uses internally
//! and it works regardless of environment state.
//!
//! ## Misconception worth recording
//!
//! `%LOCALAPPDATA%` is **`cmd.exe` syntax**. Passing the literal string
//! `"%LOCALAPPDATA%\\..."` to `std::path::Path` does **not** expand the
//! variable — Rust's path APIs go straight to Win32 file calls, which see
//! the bare `%LOCALAPPDATA%` and fail with `ENOENT`. Always read the value
//! with `std::env::var_os` (or, as a fallback, the Known Folders API).
//!
//! [`SHGetKnownFolderPath`]: https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shgetknownfolderpath

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use crate::error::ShimError;

/// Return the current user's Local AppData directory.
///
/// 1. `%LOCALAPPDATA%` — the fast path.
/// 2. `SHGetKnownFolderPath(FOLDERID_LocalAppData)` — the bulletproof
///    fallback that does not depend on the process environment.
pub fn resolve() -> Result<PathBuf, ShimError> {
    if let Some(v) = std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(v));
    }
    resolve_via_known_folder().ok_or(ShimError::LocalAppDataMissing)
}

/// `FOLDERID_LocalAppData = {F1B32785-6FBA-4FCF-9D55-7B8E7F157091}`.
const FOLDERID_LOCAL_APP_DATA: Guid = Guid {
    data1: 0xF1B3_2785,
    data2: 0x6FBA,
    data3: 0x4FCF,
    data4: [0x9D, 0x55, 0x7B, 0x8E, 0x7F, 0x15, 0x70, 0x91],
};

fn resolve_via_known_folder() -> Option<PathBuf> {
    let mut ptr: *mut u16 = std::ptr::null_mut();

    // SAFETY: Per the Win32 contract for SHGetKnownFolderPath:
    //   * `rfid` points to a valid, properly-aligned KNOWNFOLDERID — we pass
    //     a `'static` const initialized with the documented GUID layout.
    //   * `dwFlags == 0` is always valid (no special behavior requested).
    //   * `hToken == NULL` requests the *current* user, which is what we want.
    //   * `ppszPath` points to a writable `*mut u16` we own on the stack.
    // On success the OS writes a NUL-terminated UTF-16 string owned by the
    // COM task allocator into `*ppszPath`, which we must release with
    // `CoTaskMemFree`. On failure `*ppszPath` is set to NULL.
    let hr = unsafe {
        SHGetKnownFolderPath(&FOLDERID_LOCAL_APP_DATA, 0, std::ptr::null_mut(), &mut ptr)
    };

    if hr < 0 || ptr.is_null() {
        // The API still mandates CoTaskMemFree(NULL) is a no-op, so we just
        // drop our null pointer here.
        return None;
    }

    // SAFETY: `ptr` is non-null and points to a NUL-terminated UTF-16 string
    // owned by the COM allocator. We compute its length by scanning for NUL,
    // then copy out into an owned `OsString`. The wide string remains valid
    // for the duration of this unsafe block (we have not freed it yet).
    let path = unsafe {
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        let os = OsString::from_wide(slice);
        // SAFETY: `ptr` was allocated by SHGetKnownFolderPath, which
        // documents `CoTaskMemFree` as the matching deallocator.
        CoTaskMemFree(ptr.cast());
        os
    };

    Some(PathBuf::from(path))
}

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetKnownFolderPath(
        rfid: *const Guid,
        dw_flags: u32,
        h_token: *mut core::ffi::c_void,
        ppsz_path: *mut *mut u16,
    ) -> i32;
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoTaskMemFree(pv: *mut core::ffi::c_void);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_folder_resolves_to_an_existing_directory() {
        // Independent of `%LOCALAPPDATA%`: the Known Folders API must always
        // return a real directory under any normal user/session context that
        // can run a Rust test (which by definition has a profile loaded).
        let p = resolve_via_known_folder().expect("SHGetKnownFolderPath should succeed");
        assert!(
            p.is_dir(),
            "Known Folders LocalAppData path should exist as a directory: {}",
            p.display()
        );
    }

    /// Env-mutating cases are combined into a single test so they cannot
    /// race against each other under cargo's default parallel test runner.
    #[test]
    fn resolve_reads_env_then_falls_back_to_known_folder() {
        let prev = std::env::var_os("LOCALAPPDATA");

        // 1. With env set: return the env value verbatim, no `%...%`
        //    expansion, no canonicalization.
        let sentinel = std::path::PathBuf::from("C:\\__git_shim_test_sentinel__");
        // SAFETY: `set_var` is unsafe in edition 2024 due to threading
        // hazards. This test does not spawn threads of its own and is the
        // only test in the crate that mutates `LOCALAPPDATA`, so the
        // unsoundness window is closed.
        unsafe { std::env::set_var("LOCALAPPDATA", &sentinel) };
        let from_env = resolve();

        // 2. With env unset: must fall back to the Known Folders API.
        // SAFETY: see above.
        unsafe { std::env::remove_var("LOCALAPPDATA") };
        let from_fallback = resolve();

        // Restore before asserting so a failed assertion does not poison
        // any future test addition that happens to read the variable.
        match prev {
            // SAFETY: see above.
            Some(v) => unsafe { std::env::set_var("LOCALAPPDATA", v) },
            // SAFETY: see above.
            None => unsafe { std::env::remove_var("LOCALAPPDATA") },
        }

        assert_eq!(from_env.expect("env path returned"), sentinel);
        let fb = from_fallback.expect("fallback should succeed under a user session");
        assert!(fb.is_dir(), "fallback path should exist: {}", fb.display());
    }
}
