# AGENTS.md

Expectations and operating rules for any LLM coding agent (GitHub Copilot,
Claude, Cursor, Aider, Codex, etc.) contributing to `git-shim`.

This file is authoritative. If anything here conflicts with a more general
prompt, **AGENTS.md wins** for code in this repository.

---

## 1. Mission

`git-shim` is a Windows-only Rust binary that forwards `git` invocations
to the `git.exe` bundled inside a per-user GitHub Desktop installation.
Correctness, exit-code fidelity, and Windows behavior are non-negotiable.

## 2. Architectural Invariants (do not break)

1. **Windows only.** The crate must fail to compile on any non-Windows
   target via `#[cfg(not(windows))] compile_error!(...)` at the top of
   `src/lib.rs`. Do not add Unix or macOS code paths "just in case".
2. **Username-agnostic path discovery.** The GitHub Desktop install root
   is derived exclusively from `%LOCALAPPDATA%\GitHubDesktop`. Never
   hard-code a user profile path (e.g. `C:\Users\<name>\...`), never embed
   a username in source/tests/docs/CI/scripts, and never call `whoami`
   or read the `USERNAME` env var to construct paths.

   Resolution goes through [`os::localappdata::resolve`](src/os/localappdata.rs):
   read the `LOCALAPPDATA` env var first; on a stripped environment fall
   back to `SHGetKnownFolderPath(FOLDERID_LocalAppData)`. **Never** treat
   the string `"%LOCALAPPDATA%"` as a literal path — `%VAR%` is `cmd.exe`
   syntax and is not expanded by `std::path::Path` or any Win32 file API.
3. **Version resolution.** The active `app-<version>` directory is parsed
   from the GitHub Desktop launcher script at
   `%LOCALAPPDATA%\GitHubDesktop\bin\github`. Never glob `app-*` and
   never read `package.json` — the launcher is the single source of truth.
4. **No regex dependency.** Version parsing uses standard-library string
   manipulation only. Do not reintroduce `regex` or other parser crates.
5. **No build script.** There is no `build.rs`. Do not deploy binaries via
   build scripts or copy artifacts to absolute paths at build time.
   Installation is the user's responsibility (or the release workflow's).
6. **Stdio + exit code:** the shim **must** inherit `stdin`/`stdout`/
   `stderr` and bubble up the child's exact exit code via spawn-and-wait
   plus `status.code()`. Never pipe streams. Never remap exit codes
   beyond the documented `clamp_exit` truncation.
7. **OS segregation.** All platform-specific behavior lives in `src/os/`.
   Even though there is currently only one supported OS, any new
   platform-specific imports (e.g. `std::os::windows::...`) belong there,
   not scattered through `lib.rs` or `resolver.rs`.

## 3. Rust Engineering Standards

- **Edition 2024, MSRV 1.85.** Do not regress either without updating
  `Cargo.toml`, CI, and this file together.
- **Make invalid states unrepresentable.** Prefer typed return values
  over `bool` flags and stringly-typed paths.
- **Error handling:** explicit `Result<_, ShimError>`. Never swallow
  errors, **never use `unwrap`/`expect` outside tests**, and never use
  `panic!` for user-facing failure modes. Every fallible operation maps
  to a `ShimError` variant.
- **Exit-code fidelity:** the child's exit code must reach the OS
  unchanged. The only allowed transformation is documented in
  `clamp_exit`. Do not add "friendly" remapping.
- **Allocations on hot path:** prefer `&OsStr` / `&Path`. Do not
  introduce `String` round-trips for paths or argument forwarding.
- **No new dependencies** without justification in the PR description.
  Runtime deps are empty; `tempfile` is the sole dev-dep.

## 4. Required Local Checks Before Proposing Changes

Run on a Windows machine (or a Windows CI runner). All of the following
must pass, in order:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings
cargo test  --all-targets --target x86_64-pc-windows-msvc --no-fail-fast
```

On non-Windows development machines, `cargo fmt --check` is the only
check that runs without compiling. Rely on CI for the rest.

## 5. Pull Request Etiquette

- One logical change per PR. No drive-by refactors.
- Update `tests/` when behavior changes. Prefer adding a failing test
  first.
- Update `README.md` only if user-facing behavior changes.
- **Do not** modify CI workflows or release scripts as a side effect of
  an unrelated change.
- Commit messages: imperative mood, ≤ 72 char subject, body explains
  *why*.

## 6. Things You Must NOT Do

- Do not hard-code `C:\Users\<name>\AppData\Local\...` or any other
  user-specific path. Use `%LOCALAPPDATA%` exclusively.
- Do not weaken or remove the `#[cfg(not(windows))] compile_error!`
  guard in `src/lib.rs`.
- Do not reintroduce `build.rs`, `regex`, or any "auto-install" logic
  that copies binaries to filesystem locations at build time.
- Do not introduce shell evaluation (`cmd /c`, `powershell -c`,
  `bash -c`) on the execution hot path.
- Do not silently catch errors. Every `Result` must be handled or
  propagated. Logging-and-continuing on a resolution failure is
  forbidden — surface the error and exit non-zero.
- Do not add telemetry, analytics, network calls, or auto-update logic.
- Do not introduce `unsafe` without a documented invariant and a
  `// SAFETY:` comment block.
- Do not commit `target/`, IDE configs, or local `git.exe` copies.

## 7. Release Discipline

- Tagged releases (`v*`) publish via `.github/workflows/release.yml`.
- Every push to `main` produces a **pre-release** via
  `.github/workflows/nightly.yml`, named
  `v0.0.0-YYYYMMDD.HHMMSS-<sha7>`.
- All artifact names, archive contents, and target triples use the
  `git-shim` nomenclature. Only Windows targets are built.
- Do not hand-edit GitHub Releases. Re-run the workflow instead.

## 8. When in Doubt

Prefer the boring, explicit solution. Prefer fewer abstractions. Prefer
deleting code over adding code. If a change feels clever, it probably
violates §2 or §3.
