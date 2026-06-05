//! Library entry for `git-shim`. The single binary target (`git-shim`)
//! delegates to [`entry`], which resolves the active GitHub Desktop `git.exe`
//! and replaces / forwards to it.

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
    // Sanity check argv[0] exists; we do not currently dispatch on it (single-
    // mode shim) but a missing argv[0] indicates a broken host environment we
    // would rather surface than silently paper over.
    let _argv0 = std::env::args_os().next().ok_or(ShimError::MissingArgv0)?;

    let git = resolver::resolve_git()?;
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    os::exec::run(&git, &args).map_err(ShimError::Spawn)
}

/// Clamp a signed exit code to the unsigned byte that `ExitCode` accepts.
///
/// - Negative values (Unix signal-style returns) collapse to `128`, matching
///   the POSIX `128 + signo` convention without requiring us to recover the
///   signal number.
/// - Otherwise, take the low 8 bits, mirroring what `wait(2)` exposes.
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
