# AGENTS.md

Expectations and operating rules for any LLM coding agent (GitHub Copilot,
Claude, Cursor, Aider, Codex, etc.) contributing to `git-shim`.

This file is authoritative. If anything here conflicts with a more general
prompt, **AGENTS.md wins** for code in this repository.

---

## 1. Mission

`git-shim` is a stable, shell-independent Rust binary that forwards `git`
invocations to the `git.exe` bundled inside a per-user GitHub Desktop
installation. Correctness, exit-code fidelity, and Windows behavior are
non-negotiable.

## 2. Architectural Invariants (do not break)

1. **Path discovery:** the GitHub Desktop install root is derived
   exclusively from `%LOCALAPPDATA%\GitHubDesktop`. Never hard-code a
   user profile path (e.g. `C:\Users\<name>\...`) anywhere in source,
   tests, scripts, CI, or documentation.
2. **Version resolution:** the active `app-<version>` directory is parsed
   from the GitHub Desktop launcher script at
   `%LOCALAPPDATA%\GitHubDesktop\bin\github`. Never globbing
   `app-*` and never reading `package.json` — the launcher is the single
   source of truth.
3. **No regex dependency.** Version parsing uses standard-library string
   manipulation only. Do not reintroduce `regex` or other parser crates.
4. **No build script.** There is no `build.rs`. Do not deploy binaries
   via build scripts or copy artifacts to absolute paths at build time.
   Installation is the user's responsibility (or the release workflow's).
5. **Stdio + exit code:** the shim **must** inherit `stdin`/`stdout`/
   `stderr` and bubble up the child's exact exit code. On Unix this means
   `CommandExt::exec` (process replacement). On Windows, spawn-and-wait
   and forward `status.code()` byte-for-byte.
6. **OS segregation:** every platform-specific behavior lives behind
   `#[cfg(target_os = "...")]` / `#[cfg(unix)]` / `#[cfg(windows)]` in
   `src/os/` or in a `cfg`-gated `imp` submodule. No `cfg!()` runtime
   checks for OS dispatch on the hot path.
7. **Single mode.** `git-shim` is not a multicall binary. If a future
   change requires multicall dispatch, model it as a typed `Mode` enum
   parsed from `argv[0]` — never branch on environment variables.

## 3. Rust Engineering Standards

- **Edition 2024, MSRV 1.85.** Do not regress either without updating
  `Cargo.toml`, CI, and this file together.
- **Make invalid states unrepresentable.** Prefer typed return values
  over `bool` flags and stringly-typed paths.
- **Error handling:** explicit `Result<_, ShimError>`. Never swallow
  errors, **never use `unwrap`/`expect` outside tests**, and never use
  `panic!` for user-facing failure modes. Every fallible operation maps
  to a `ShimError` variant.
- **Exit code fidelity:** the child's exit code must reach the OS
  unchanged. The only allowed transformations are documented in
  `clamp_exit`. Do not add "friendly" remapping.
- **Allocations on hot path:** prefer `&OsStr` / `&Path`. Do not
  introduce `String` round-trips for paths or argument forwarding.
- **No new dependencies** without justification in the PR description.
  The current set is empty for runtime and `tempfile` for dev.

## 4. Required Local Checks Before Proposing Changes

All of the following must pass, in order:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

If GitHub Desktop is not installed on the development machine, the
Windows-only resolver path cannot be exercised end-to-end; rely on the
unit tests in `src/resolver.rs` and on CI for runtime coverage.

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
- Do not reintroduce `build.rs`, `regex`, or any "auto-install" logic
  that copies binaries to filesystem locations at build time.
- Do not introduce shell evaluation (`bash -c`, `cmd /c`,
  `powershell -c`) on the execution hot path.
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
- All artifact names, archive contents, and target triples must use the
  `git-shim` nomenclature.
- Do not hand-edit GitHub Releases. Re-run the workflow instead.

## 8. When in Doubt

Prefer the boring, explicit solution. Prefer fewer abstractions. Prefer
deleting code over adding code. If a change feels clever, it probably
violates §2 or §3.
