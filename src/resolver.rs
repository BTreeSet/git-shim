//! Resolve the absolute path to the `git.exe` shipped inside the current
//! user's GitHub Desktop installation.
//!
//! ## Layout assumptions
//!
//! GitHub Desktop on Windows installs per-user under `%LOCALAPPDATA%`:
//!
//! ```text
//! %LOCALAPPDATA%\GitHubDesktop\
//!     bin\
//!         github                 (POSIX shell script launcher)
//!     app-<version>\
//!         resources\app\git\cmd\git.exe
//! ```
//!
//! The launcher script embeds the current `app-<version>` directory in its
//! body. Parsing that token lets us locate the active install without
//! hard-coding a version that changes on every GitHub Desktop update — and
//! without ever referring to a specific user profile name.
//!
//! ## Why parse the launcher and not glob `app-*`?
//!
//! Multiple `app-*` directories can coexist briefly during an update. The
//! launcher script is the single source of truth for which one is currently
//! active.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::error::ShimError;
use crate::os::localappdata;

/// Token that always precedes the `app-<version>` segment in the launcher
/// script body, regardless of how the user's `%LOCALAPPDATA%` is spelled.
const LAUNCHER_NEEDLE: &str = "/resources/app/static/github.sh";

/// Resolve the active `git.exe` path for the current user's GitHub Desktop
/// install. The returned path is canonicalized and verified to exist.
///
/// ## Why we keep the `\\?\` prefix
///
/// `Path::canonicalize` on Windows returns *extended-length* form
/// (`\\?\C:\...` or `\\?\UNC\server\share\...`). Win32 file APIs —
/// including `CreateProcessW`, which is what `Command::new(...).status()`
/// eventually invokes — **prefer** this form: it bypasses the `MAX_PATH`
/// limit and skips the kernel's path-normalization pass. We therefore
/// return the canonical form **unchanged** from the resolver and only
/// strip the prefix at human-facing display boundaries via
/// [`display_path`] (the debug knob's `println!`, [`ShimError`] messages,
/// etc.).
pub fn resolve_git() -> Result<PathBuf, ShimError> {
    // `localappdata::resolve` reads the `LOCALAPPDATA` env var first and
    // falls back to the Win32 Known Folders API. We never treat
    // `"%LOCALAPPDATA%"` as a literal path — that's `cmd.exe` syntax, not a
    // Rust/Win32 path concept.
    let local_app_data = localappdata::resolve()?;

    let install_root = PathBuf::from(&local_app_data).join("GitHubDesktop");
    let launcher = install_root.join("bin").join("github");

    if !launcher.is_file() {
        return Err(ShimError::LauncherMissing(launcher));
    }

    let contents = std::fs::read_to_string(&launcher)
        .map_err(|e| ShimError::LauncherRead(launcher.clone(), e))?;

    let app_version = parse_app_version(&contents)
        .ok_or_else(|| ShimError::VersionTokenMissing(launcher.clone()))?;

    let candidate = git_path_for(&install_root, app_version);
    let canonical = candidate
        .canonicalize()
        .map_err(|e| ShimError::CanonicalizeFailed(candidate.clone(), e))?;

    // `Path::is_file` accepts extended-length paths natively, so we can
    // verify existence without first stripping the prefix.
    if !canonical.is_file() {
        return Err(ShimError::GitExecutableMissing(canonical));
    }
    Ok(canonical)
}

/// Return a human-displayable view of `p` with any Win32 extended-length
/// prefix removed.
///
/// Use this **only** at display boundaries — `println!`, error messages,
/// log lines, things compared by the e2e script. **Never** use it to
/// derive a path passed to a Win32 API; the extended-length form is the
/// preferred input there.
///
/// Returns the input borrowed unchanged when no prefix is present, so the
/// no-strip case allocates nothing.
///
/// | Input                              | Output                       |
/// |------------------------------------|------------------------------|
/// | `\\?\C:\Users\foo`                 | `C:\Users\foo`               |
/// | `\\?\UNC\server\share\foo`         | `\\server\share\foo`         |
/// | `\\?\Volume{GUID}\...`             | unchanged (no shorter form)  |
/// | `C:\already\normal`                | unchanged                    |
///
/// Equivalent in spirit to `dunce::simplified`, inlined to avoid a
/// dependency.
pub fn display_path(p: &Path) -> Cow<'_, Path> {
    // We compare on `&str` because the prefixes are pure ASCII. If the
    // path is not valid UTF-8 it cannot start with our ASCII prefixes
    // either, so returning it unchanged is correct.
    let Some(s) = p.to_str() else {
        return Cow::Borrowed(p);
    };

    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        // Restore the UNC `\\server\share\...` form.
        let mut out = String::with_capacity(rest.len() + 2);
        out.push_str(r"\\");
        out.push_str(rest);
        return Cow::Owned(PathBuf::from(out));
    }

    if let Some(rest) = s.strip_prefix(r"\\?\") {
        // Only strip for plain drive-letter paths, e.g. `C:\...`. Volume
        // GUID paths (`Volume{...}`) and device paths have no shorter
        // equivalent form.
        let bytes = rest.as_bytes();
        if bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == b'\\'
        {
            return Cow::Owned(PathBuf::from(rest));
        }
    }

    Cow::Borrowed(p)
}

/// Pure helper: given the textual contents of the GitHub Desktop launcher
/// script, extract the `app-<version>` directory name embedded in it.
///
/// Returns `None` if the expected token is missing or malformed.
///
/// This is the standard-library replacement for the previous `regex`
/// dependency. The launcher embeds a POSIX-style path of the form
/// `.../app-3.4.5/resources/app/static/github.sh`. We locate the suffix and
/// walk backwards to the preceding `/` to slice out the directory name.
pub fn parse_app_version(launcher_contents: &str) -> Option<&str> {
    let suffix_idx = launcher_contents.find(LAUNCHER_NEEDLE)?;
    // Everything before the suffix; the segment we want is the last `/`-
    // delimited component of that prefix.
    let prefix = &launcher_contents[..suffix_idx];
    let segment_start = prefix.rfind('/')? + 1;
    let segment = &prefix[segment_start..];
    if segment.starts_with("app-") && segment.len() > "app-".len() {
        Some(segment)
    } else {
        None
    }
}

/// Build the expected `git.exe` path beneath a GitHub Desktop install root,
/// given the resolved `app-<version>` directory name. Pure path arithmetic;
/// does not touch the filesystem.
pub fn git_path_for(install_root: &Path, app_version: &str) -> PathBuf {
    install_root
        .join(app_version)
        .join("resources")
        .join("app")
        .join("git")
        .join("cmd")
        .join("git.exe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typical_launcher_body() {
        // The path inside the launcher embeds whatever user profile the
        // installer ran under. We deliberately use a generic name in tests
        // to assert that the parser is username-agnostic.
        let body = r#"#!/bin/sh
exec "/c/Users/example/AppData/Local/GitHubDesktop/app-3.4.5/resources/app/static/github.sh" "$@"
"#;
        assert_eq!(parse_app_version(body), Some("app-3.4.5"));
    }

    #[test]
    fn parses_version_with_dashes_and_digits() {
        let body =
            "anything /Users/x/Local/GitHubDesktop/app-3.5.0-beta1/resources/app/static/github.sh";
        assert_eq!(parse_app_version(body), Some("app-3.5.0-beta1"));
    }

    #[test]
    fn parser_is_username_agnostic() {
        for user in ["alice", "bob.smith", "carol-dev", "ARK Builder"] {
            let body = format!(
                "exec \"/c/Users/{user}/AppData/Local/GitHubDesktop/app-9.9.9/resources/app/static/github.sh\""
            );
            assert_eq!(parse_app_version(&body), Some("app-9.9.9"), "user={user}");
        }
    }

    #[test]
    fn rejects_missing_needle() {
        assert!(parse_app_version("nothing relevant here").is_none());
    }

    #[test]
    fn rejects_segment_without_app_prefix() {
        let body = "/foo/bar/baz-1.0/resources/app/static/github.sh";
        assert!(parse_app_version(body).is_none());
    }

    #[test]
    fn rejects_bare_app_segment() {
        let body = "/foo/bar/app-/resources/app/static/github.sh";
        assert!(parse_app_version(body).is_none());
    }

    #[test]
    fn rejects_segment_without_leading_slash() {
        // No leading `/` before `app-X`; rfind would return None.
        assert!(parse_app_version("app-3.4.5/resources/app/static/github.sh").is_none());
    }

    #[test]
    fn picks_last_matching_segment_when_path_is_nested() {
        // `rfind('/')` ensures we take the segment immediately preceding the
        // needle, not some earlier one.
        let body = "/old/app-1.0.0/x/app-2.0.0/resources/app/static/github.sh";
        assert_eq!(parse_app_version(body), Some("app-2.0.0"));
    }

    #[test]
    fn git_path_for_assembles_expected_layout() {
        let root = std::path::Path::new("C:/install");
        let p = git_path_for(root, "app-3.4.5");
        let expected: std::path::PathBuf = [
            "C:/install",
            "app-3.4.5",
            "resources",
            "app",
            "git",
            "cmd",
            "git.exe",
        ]
        .iter()
        .collect();
        assert_eq!(p, expected);
    }

    #[test]
    fn display_path_removes_extended_drive_letter_form() {
        let p = Path::new(
            r"\\?\C:\Users\runneradmin\AppData\Local\GitHubDesktop\app-3.5.12\resources\app\git\cmd\git.exe",
        );
        let got = display_path(p);
        assert!(matches!(got, Cow::Owned(_)), "prefix strip must allocate");
        assert_eq!(
            &*got,
            Path::new(
                r"C:\Users\runneradmin\AppData\Local\GitHubDesktop\app-3.5.12\resources\app\git\cmd\git.exe"
            )
        );
    }

    #[test]
    fn display_path_rewrites_unc_form() {
        let p = Path::new(r"\\?\UNC\server\share\dir\file.txt");
        let got = display_path(p);
        assert!(matches!(got, Cow::Owned(_)));
        assert_eq!(&*got, Path::new(r"\\server\share\dir\file.txt"));
    }

    #[test]
    fn display_path_leaves_volume_guid_paths_alone() {
        // Volume GUID paths have no shorter, equivalent form.
        let p = Path::new(r"\\?\Volume{12345678-1234-1234-1234-1234567890ab}\foo\bar");
        let got = display_path(p);
        assert!(
            matches!(got, Cow::Borrowed(_)),
            "volume-GUID path must be returned borrowed/unchanged"
        );
        assert_eq!(&*got, p);
    }

    #[test]
    fn display_path_passes_through_already_normal_paths() {
        let p = Path::new(r"C:\Program Files\Git\cmd\git.exe");
        let got = display_path(p);
        assert!(
            matches!(got, Cow::Borrowed(_)),
            "no allocation for plain paths"
        );
        assert_eq!(&*got, p);
    }

    #[test]
    fn display_path_passes_through_relative_paths() {
        let p = Path::new(r"some\relative\path");
        let got = display_path(p);
        assert!(matches!(got, Cow::Borrowed(_)));
        assert_eq!(&*got, p);
    }
}
