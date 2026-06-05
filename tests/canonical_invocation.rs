//! Integration test: verifies the canonical `git-shim` binary errors clearly
//! on platforms where GitHub Desktop has no supported install layout (Linux,
//! macOS), or when the install simply isn't present.
//!
//! This exercises the full `main()` pipeline without requiring GitHub
//! Desktop or `git.exe` to be installed on the test runner.

use std::process::Command;

fn shim_path() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target");
    p.push(if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    });
    p.push(if cfg!(windows) {
        "git-shim.exe"
    } else {
        "git-shim"
    });
    p
}

#[test]
fn shim_exits_nonzero_when_resolution_fails() {
    let bin = shim_path();
    if !bin.exists() {
        let status = Command::new(env!("CARGO"))
            .args(["build", "--bin", "git-shim"])
            .status()
            .expect("cargo build");
        assert!(status.success(), "failed to build git-shim bin");
    }
    assert!(bin.exists(), "expected built binary at {}", bin.display());

    // On Unix the resolver returns `UnsupportedPlatform`. On Windows without
    // GitHub Desktop installed it returns `LauncherMissing` (or, in the
    // unlikely event of an unset `%LOCALAPPDATA%`, `LocalAppDataMissing`).
    // In every case we expect a non-zero exit and a `git-shim:` diagnostic.
    let mut cmd = Command::new(&bin);
    // Scrub `LOCALAPPDATA` so Windows runners exercise a deterministic
    // failure path even if some unrelated directory happens to exist.
    cmd.env_remove("LOCALAPPDATA");

    let out = cmd.output().expect("spawn git-shim");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "expected non-zero exit, got success with stderr: {stderr}"
    );
    assert!(
        stderr.contains("git-shim:"),
        "expected `git-shim:` diagnostic on stderr, got: {stderr}"
    );
}
