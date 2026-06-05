//! Integration test: verifies the canonical `git-shim` binary errors clearly
//! when GitHub Desktop cannot be located, by scrubbing `%LOCALAPPDATA%` so
//! the resolver hits a deterministic failure path.
//!
//! This exercises the full `main()` pipeline without requiring an actual
//! GitHub Desktop install on the test runner.

use std::process::Command;

fn shim_path() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target");
    p.push(if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    });
    p.push("git-shim.exe");
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

    // Scrub `%LOCALAPPDATA%` so the resolver fails deterministically with
    // `LocalAppDataMissing` even if GitHub Desktop happens to be installed
    // on the runner.
    let out = Command::new(&bin)
        .env_remove("LOCALAPPDATA")
        .output()
        .expect("spawn git-shim");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "expected non-zero exit, got success with stderr: {stderr}"
    );
    assert!(
        stderr.contains("git-shim:"),
        "expected `git-shim:` diagnostic on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("LOCALAPPDATA"),
        "expected LOCALAPPDATA mention on stderr, got: {stderr}"
    );
}
