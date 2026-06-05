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
7. **Canonical paths internally, stripped only for display.**
   `Path::canonicalize` on Windows returns extended-length form
   (`\\?\C:\...` or `\\?\UNC\server\share\...`). That is the **preferred**
   input shape for `CreateProcessW` and other Win32 file APIs (it bypasses
   `MAX_PATH` and skips kernel re-normalization), so the resolver returns
   it **unchanged** and the exec layer passes it straight through.
   The prefix is stripped **only at human-display boundaries** via
   `resolver::display_path`: the `GIT_SHIM_PRINT_RESOLVED` debug print,
   every path embedded in a `ShimError::Display` message, and any future
   log line. Do not pre-strip in the resolver. Do not skip stripping at
   the display sites. Volume-GUID paths (`\\?\Volume{...}`) have no
   shorter form and are passed through unchanged by `display_path`.
8. **OS segregation.** All platform-specific behavior lives in `src/os/`.
   Even though there is currently only one supported OS, any new
   platform-specific imports (e.g. `std::os::windows::...`) belong there,
   not scattered through `lib.rs` or `resolver.rs`.
9. **Debug interface.** The shim recognizes exactly one out-of-band
   environment variable: `GIT_SHIM_PRINT_RESOLVED`. When set to a
   non-empty value, the shim resolves the GitHub Desktop `git.exe`,
   prints its absolute path to stdout, and exits `0` **without invoking
   git**. This variable exists for the e2e CI job
   (`scripts/e2e.ps1` invoked from `.github/workflows/ci.yml`) and must
   not be removed, renamed, or repurposed. Do not add additional
   `GIT_SHIM_*` knobs without strong justification — every knob is an
   exception to "behave exactly like git".

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

## 8. CI Privilege Discipline (least privilege, structurally enforced)

Every workflow under `.github/workflows/` MUST follow these rules:

1. **Deny-by-default at the workflow level.** The top of every workflow
   declares `permissions: {}`. This zeroes out every implicit scope —
   most importantly `packages: write`, which older repository defaults
   grant — regardless of repository-level Actions settings.
2. **Per-job re-elevation, minimum scope.** Each job declares its own
   `permissions:` block listing only the scopes it actually uses. Read
   jobs get `contents: read`; the single release-upload job gets
   `contents: write`. No job ever needs `packages: *` — we publish to
   GitHub Releases, not the package registry.
3. **No write tokens in jobs that compile or run third-party code.**
   The `build`, `gate`, `test`, `lint`, and `msrv` jobs all execute
   third-party cargo dependencies (build scripts, proc-macros). They
   are restricted to `contents: read` and use
   `actions/checkout` with `persist-credentials: false` so the token
   is not left in `.git/config` for a downstream process to harvest.
4. **The `e2e` job is the most sensitive surface.** It downloads and
   executes an *external* installer (GitHub Desktop). It MUST keep
   `permissions: contents: read` and `persist-credentials: false`, and
   MUST NOT receive `GITHUB_TOKEN` via any explicit `env:` block.
5. **Publish jobs are dedicated and minimal.** The job with
   `contents: write` does not check out the source tree, does not run
   cargo, and only downloads pre-built artifacts before invoking the
   release action. This structurally prevents any third-party code
   path from observing the elevated token.
6. **No new permission scopes without justification.** Adding
   `id-token`, `packages`, `pages`, `pull-requests`, etc. to any job
   requires a PR description explaining the exact API call that needs
   it. The default answer is "no".

## 9. When in Doubt

Prefer the boring, explicit solution. Prefer fewer abstractions. Prefer
deleting code over adding code. If a change feels clever, it probably
violates §2 or §3.
