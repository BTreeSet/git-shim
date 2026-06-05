# git-shim

A stable, shell-independent Rust shim that forwards `git` invocations to the
`git.exe` bundled inside a per-user [GitHub Desktop] installation — without
requiring the user to add Git for Windows separately to `%PATH%`.

[GitHub Desktop]: https://desktop.github.com/

> **Why?** GitHub Desktop ships a full Git for Windows distribution under
> `%LOCALAPPDATA%\GitHubDesktop\app-<version>\resources\app\git\cmd\git.exe`,
> but the directory name changes on every GitHub Desktop update. `git-shim`
> resolves the currently-active install at runtime by parsing the launcher
> script that GitHub Desktop itself maintains, then forwards the call —
> inheriting stdio and preserving the exit code byte-for-byte.

## Install

1. Build the release binary or download a prebuilt artifact:

   ```sh
   cargo build --release
   ```

2. Place `git-shim.exe` somewhere on your `%PATH%` *under the name `git.exe`*
   (a copy, hardlink, or symlink works):

   ```powershell
   $shim = "$Env:USERPROFILE\.git-shim\git.exe"
   New-Item -ItemType Directory -Force -Path (Split-Path $shim) | Out-Null
   Copy-Item -Force .\target\release\git-shim.exe $shim
   # Prepend to PATH (current session):
   $Env:PATH = (Split-Path $shim) + ";" + $Env:PATH
   ```

3. Verify:

   ```powershell
   git --version
   ```

## How it works

1. Read `%LOCALAPPDATA%\GitHubDesktop\bin\github` (a POSIX shell script
   GitHub Desktop maintains).
2. Extract the embedded `app-<version>` token via standard-library string
   manipulation (no `regex` dependency).
3. Canonicalize
   `%LOCALAPPDATA%\GitHubDesktop\app-<version>\resources\app\git\cmd\git.exe`.
4. Forward all CLI arguments to it, inherit stdio, and bubble up the exact
   exit code.

## Architectural Invariants

- No hard-coded user profile paths — `%LOCALAPPDATA%` only.
- No `build.rs`, no `regex`, no automatic file-copy side effects.
- On Unix the shim refuses to silently fall back to system `git`; it surfaces
  `UnsupportedPlatform`. GitHub Desktop has no comparable layout outside
  Windows.

See [`AGENTS.md`](AGENTS.md) for the full set of contributor invariants.

## License

MIT
