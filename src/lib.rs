//! Library entry for `git-shim`. The single binary target (`git-shim`)
//! delegates to [`entry`], which resolves the active GitHub Desktop `git.exe`
//! and forwards to it.
//!
//! ## Platform scope
//!
//! `git-shim` only makes sense on Windows: GitHub Desktop ships a Git for
//! Windows distribution inside `%LOCALAPPDATA%\GitHubDesktop\app-<version>\`
//! that has no analogue on macOS or Linux. The crate therefore refuses to
//! compile on any other target.
//!
//! ## Debug knob: `GIT_SHIM_PRINT_RESOLVED`
//!
//! When the environment variable `GIT_SHIM_PRINT_RESOLVED` is set to any
//! non-empty value, the shim resolves the GitHub Desktop `git.exe` path,
//! prints it to standard output, and exits with status `0` **without
//! invoking git**. This intentionally deviates from real git CLI behavior
//! and exists purely so the e2e CI job can verify the shim picks GitHub
//! Desktop's bundled git rather than any system git on `%PATH%`.
//!
//! A name prefixed with `GIT_SHIM_` cannot collide with any real git
//! environment variable (git uses `GIT_*`, `GIT_CONFIG_*`, etc.).

#[cfg(not(windows))]
compile_error!(
    "git-shim only supports Windows. GitHub Desktop's bundled git.exe lives \
     under %LOCALAPPDATA%\\GitHubDesktop\\app-<version>\\..., a layout that \
     does not exist on macOS or Linux. Build with --target \
     x86_64-pc-windows-msvc or aarch64-pc-windows-msvc."
);

pub mod error;
pub mod os;
pub mod resolver;

use std::process::ExitCode;

use crate::error::ShimError;

/// Library-level entry invoked by the thin `main` wrapper.
///
/// Translates any [`ShimError`] into a stable exit code (127) and prints a
/// single-line diagnostic to stderr. On success, returns the child's exit
/// code clamped to a `u8` per `ExitCode`'s contract.
pub fn entry() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(clamp_exit(code)),
        Err(err) => {
            eprintln!("git-shim: {err}");
            ExitCode::from(127)
        }
    }
}

fn run() -> Result<i32, ShimError> {
    // Sanity check argv[0] exists. We do not currently dispatch on it
    // (single-mode shim) but a missing argv[0] indicates a broken host
    // environment we would rather surface than silently paper over.
    let _argv0 = std::env::args_os().next().ok_or(ShimError::MissingArgv0)?;

    let git = resolver::resolve_git()?;

    // Debug knob (see crate-level docs): print the resolved git.exe and
    // exit before spawning. Used by the e2e CI job to verify the resolver
    // points at GitHub Desktop's bundled git, not the runner's system git.
    //
    // We strip the Win32 `\\?\` extended-length prefix here (a display
    // boundary) so the output is the form humans and external tools
    // expect. The canonical form is still what gets passed to
    // `os::exec::run` below — `CreateProcessW` prefers it.
    if std::env::var_os("GIT_SHIM_PRINT_RESOLVED").is_some_and(|v| !v.is_empty()) {
        println!("{}", resolver::display_path(&git).display());
        return Ok(0);
    }

    // Hand the args iterator directly to `os::exec::run`; no intermediate
    // `Vec<OsString>` allocation. `Command::args` consumes the iterator
    // lazily, copying each `OsStr` straight into the child command line.
    os::exec::run(&git, std::env::args_os().skip(1)).map_err(ShimError::Spawn)
}

/// Clamp a signed exit code to the unsigned byte that `ExitCode` accepts.
///
/// - Negative values collapse to `128`. They cannot originate from a normal
///   Windows child exit, but we defend in depth in case a future code path
///   ever returns one.
/// - Otherwise, take the low 8 bits, mirroring POSIX `wait(2)` semantics
///   that callers of CLI tools commonly assume.
#[inline]
pub fn clamp_exit(code: i32) -> u8 {
    if code < 0 { 128 } else { (code & 0xFF) as u8 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_exit_passes_through_small_codes() {
        assert_eq!(clamp_exit(0), 0);
        assert_eq!(clamp_exit(1), 1);
        assert_eq!(clamp_exit(42), 42);
        assert_eq!(clamp_exit(255), 255);
    }

    #[test]
    fn clamp_exit_truncates_high_codes() {
        assert_eq!(clamp_exit(256), 0);
        assert_eq!(clamp_exit(257), 1);
    }

    #[test]
    fn clamp_exit_maps_negative_to_128() {
        assert_eq!(clamp_exit(-1), 128);
        assert_eq!(clamp_exit(-15), 128);
    }
}
