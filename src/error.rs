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
        match self {
            ShimError::MissingArgv0 => write!(f, "argv[0] was not provided by the OS"),
            ShimError::LocalAppDataMissing => write!(
                f,
                "%LOCALAPPDATA% is not set; cannot locate GitHub Desktop install root"
            ),
            ShimError::LauncherMissing(p) => write!(
                f,
                "GitHub Desktop launcher script not found at {} (is GitHub Desktop installed?)",
                p.display()
            ),
            ShimError::LauncherRead(p, e) => {
                write!(f, "failed to read launcher script {}: {e}", p.display())
            }
            ShimError::VersionTokenMissing(p) => write!(
                f,
                "could not parse `app-<version>` token from {} \
                 (GitHub Desktop layout may have changed)",
                p.display()
            ),
            ShimError::GitExecutableMissing(p) => {
                write!(f, "resolved git executable does not exist: {}", p.display())
            }
            ShimError::CanonicalizeFailed(p, e) => {
                write!(f, "failed to canonicalize {}: {e}", p.display())
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
