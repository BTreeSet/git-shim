//! Cross-platform child process execution that transparently forwards stdio
//! and surfaces the exact exit code.
//!
//! On Unix we replace the current process image via
//! [`std::os::unix::process::CommandExt::exec`] so signals, PID, and parent
//! relationship are preserved. On Windows the OS does not provide a true
//! `execve`, so we spawn-and-wait and forward the child's exit code
//! byte-for-byte.

use std::ffi::OsString;
use std::path::Path;

/// Run `program` with `args`, inheriting `stdin`/`stdout`/`stderr`.
///
/// Returns the child's exit code. On Unix this only returns on failure to
/// `exec`; on success the process is replaced and control never returns.
pub fn run(program: &Path, args: &[OsString]) -> std::io::Result<i32> {
    imp::run(program, args)
}

#[cfg(unix)]
mod imp {
    use std::ffi::OsString;
    use std::os::unix::process::CommandExt;
    use std::path::Path;
    use std::process::Command;

    pub fn run(program: &Path, args: &[OsString]) -> std::io::Result<i32> {
        // `exec` replaces the current process on success and only returns on
        // failure. This preserves PID, signal delivery, and avoids the
        // wait() round-trip that would otherwise distort exit semantics.
        let err = Command::new(program).args(args).exec();
        Err(err)
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::OsString;
    use std::path::Path;
    use std::process::Command;

    pub fn run(program: &Path, args: &[OsString]) -> std::io::Result<i32> {
        // The caller has already resolved an absolute path with `.exe`, so
        // CreateProcess will accept it directly. stdio is inherited by
        // default; we deliberately never pipe.
        let status = Command::new(program).args(args).status()?;
        // On Windows `ExitStatus::code()` is `Some` for normal termination.
        // A `None` only appears for non-standard OS-reported statuses; map
        // that deterministically to 1 rather than panic.
        Ok(status.code().unwrap_or(1))
    }
}
