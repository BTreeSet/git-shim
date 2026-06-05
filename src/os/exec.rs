//! Windows-only child process execution that transparently forwards stdio
//! and surfaces the exact exit code.
//!
//! Windows does not provide a true `execve`, so we spawn-and-wait and
//! forward the child's exit code byte-for-byte. `stdin`/`stdout`/`stderr`
//! are inherited by default; we deliberately never pipe.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

/// Run `program` with `args`, inheriting `stdin`/`stdout`/`stderr`. Returns
/// the child's exit code.
///
/// `args` is taken as an `IntoIterator<Item: AsRef<OsStr>>` to mirror
/// `Command::args`'s contract exactly. This lets the caller hand off
/// `env::args_os().skip(1)` directly without materializing an intermediate
/// `Vec<OsString>` — monomorphization makes this strictly zero-cost.
pub fn run<I, S>(program: &Path, args: I) -> std::io::Result<i32>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    // The caller has already resolved an absolute path with `.exe`, so
    // `CreateProcess` will accept it directly.
    let status = Command::new(program).args(args).status()?;
    // On Windows `ExitStatus::code()` is `Some` for normal termination. A
    // `None` only appears for non-standard OS-reported statuses; map that
    // deterministically to 1 rather than panic.
    Ok(status.code().unwrap_or(1))
}
