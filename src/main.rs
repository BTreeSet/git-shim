//! Thin binary wrapper. All logic lives in the library so it can be tested
//! without spawning a subprocess.

fn main() -> std::process::ExitCode {
    git_shim::entry()
}
