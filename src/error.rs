//! Explicit, typed failure modes for `git-shim`.
//!
//! Every fallible operation in the crate returns `Result<_, ShimError>`. We
//! intentionally avoid `Box<dyn Error>` so callers (and tests) can match on
//! exact variants and surface stable error messages.

use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ShimError {
    /// The host OS did not provide `argv[0]`. Should be unreachable on any
    /// supported platform, but we model it rather than panic.
    MissingArgv0,
    /// `%LOCALAPPDATA%` is unset or empty; cannot derive the install root.
    /// In practice this only happens when the variable has been explicitly
    /// scrubbed (e.g. by a misconfigured service or a test harness).
    LocalAppDataMissing,
    /// The GitHub Desktop launcher script could not be found at the expected
    /// path. Usually means GitHub Desktop is not installed for this user.
    LauncherMissing(PathBuf),
    /// Failed to read the launcher script.
    LauncherRead(PathBuf, io::Error),
    /// The launcher script did not contain a parseable `app-<version>` token.
    VersionTokenMissing(PathBuf),
    /// The resolved `git.exe` path does not exist or is not a file.
    GitExecutableMissing(PathBuf),
    /// The resolved `git.exe` could not be canonicalized.
    CanonicalizeFailed(PathBuf, io::Error),
    /// Spawning / waiting on the child process failed.
    Spawn(io::Error),
}

impl fmt::Display for ShimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // All `ShimError` variants that carry a path are user-facing, so we
        // route every path through `resolver::display_path` to strip Win32
        // extended-length (`\\?\`) prefixes before formatting. Anything
        // that needs the canonical form for a Win32 API call must use the
        // raw `PathBuf` field directly, not the `Display` output.
        use crate::resolver::display_path;
        match self {
            ShimError::MissingArgv0 => write!(f, "argv[0] was not provided by the OS"),
            ShimError::LocalAppDataMissing => write!(
                f,
                "%LOCALAPPDATA% is not set; cannot locate GitHub Desktop install root"
            ),
            ShimError::LauncherMissing(p) => write!(
                f,
                "GitHub Desktop launcher script not found at {} (is GitHub Desktop installed?)",
                display_path(p).display()
            ),
            ShimError::LauncherRead(p, e) => {
                write!(
                    f,
                    "failed to read launcher script {}: {e}",
                    display_path(p).display()
                )
            }
            ShimError::VersionTokenMissing(p) => write!(
                f,
                "could not parse `app-<version>` token from {} \
                 (GitHub Desktop layout may have changed)",
                display_path(p).display()
            ),
            ShimError::GitExecutableMissing(p) => {
                write!(
                    f,
                    "resolved git executable does not exist: {}",
                    display_path(p).display()
                )
            }
            ShimError::CanonicalizeFailed(p, e) => {
                write!(
                    f,
                    "failed to canonicalize {}: {e}",
                    display_path(p).display()
                )
            }
            ShimError::Spawn(e) => write!(f, "failed to spawn git: {e}"),
        }
    }
}

impl std::error::Error for ShimError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ShimError::LauncherRead(_, e)
            | ShimError::CanonicalizeFailed(_, e)
            | ShimError::Spawn(e) => Some(e),
            _ => None,
        }
    }
}
