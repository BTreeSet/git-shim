# git-shim

A Windows-only Rust shim that forwards `git` invocations to the `git.exe`
bundled inside a per-user [GitHub Desktop] installation — without requiring
the user to add Git for Windows separately to `%PATH%`.

[GitHub Desktop]: https://desktop.github.com/

> **Why?** GitHub Desktop ships a full Git for Windows distribution under
> `%LOCALAPPDATA%\GitHubDesktop\app-<version>\resources\app\git\cmd\git.exe`,
> but the directory name changes on every GitHub Desktop update. `git-shim`
> resolves the currently-active install at runtime by parsing the launcher
> script that GitHub Desktop itself maintains, then forwards the call —
> inheriting stdio and preserving the exit code byte-for-byte.

## Platform support

**Windows only.** GitHub Desktop's on-disk layout under `%LOCALAPPDATA%` has
no analogue on macOS or Linux, so `git-shim` refuses to compile for any
non-Windows target (`compile_error!` in `src/lib.rs`). Supported targets:

- `x86_64-pc-windows-msvc`
- `aarch64-pc-windows-msvc`

## How it locates `git.exe` (no usernames involved)

`git-shim` derives every path it touches from the standard `%LOCALAPPDATA%`
environment variable, which Windows sets per user. It never reads, embeds,
or assumes a specific username — so the same compiled binary works for any
user on any machine.

1. Read `%LOCALAPPDATA%\GitHubDesktop\bin\github` (a POSIX shell script
   GitHub Desktop maintains).
2. Extract the embedded `app-<version>` token via standard-library string
   manipulation (no `regex` dependency).
3. Canonicalize
   `%LOCALAPPDATA%\GitHubDesktop\app-<version>\resources\app\git\cmd\git.exe`.
4. Forward all CLI arguments, inherit stdio, and bubble up the exact exit
   code.

## Install

1. Build the release binary (on Windows) or download a prebuilt artifact:

   ```powershell
   cargo build --release --target x86_64-pc-windows-msvc
   ```

2. Place `git-shim.exe` somewhere on your `%PATH%` *under the name `git.exe`*
   (a copy, hardlink, or symlink works):

   ```powershell
   $shim = "$Env:USERPROFILE\.git-shim\git.exe"
   New-Item -ItemType Directory -Force -Path (Split-Path $shim) | Out-Null
   Copy-Item -Force .\target\x86_64-pc-windows-msvc\release\git-shim.exe $shim
   # Prepend to PATH (current session):
   $Env:PATH = (Split-Path $shim) + ";" + $Env:PATH
   ```

3. Verify:

   ```powershell
   git --version
   ```

## Architectural Invariants

- Windows-only at the type-system level (`#[cfg(not(windows))] compile_error!`).
- No hard-coded user profile paths — `%LOCALAPPDATA%` is the only path source.
- No `build.rs`, no `regex`, no automatic file-copy side effects.

See [AGENTS.md](AGENTS.md) for the full set of contributor invariants.

## License

MIT
