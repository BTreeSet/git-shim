# Requires -Version 5.1
# End-to-end verification for git-shim against a real GitHub Desktop install.
#
# Run AFTER GitHub Desktop has been installed for the current user (see
# install-github-desktop.ps1). Builds the shim, then asserts:
#
#   1. With %LOCALAPPDATA% set, the shim resolves to
#      %LOCALAPPDATA%\GitHubDesktop\app-*\resources\app\git\cmd\git.exe
#   2. The resolved path is NOT the runner's system git on %PATH%.
#   3. With %LOCALAPPDATA% cleared, the Known Folders fallback resolves
#      to the SAME git.exe.
#   4. `git-shim.exe --version` runs and prints a git version banner that
#      matches GitHub Desktop's bundled git.
#
# Exits non-zero on any failure. Intended to be CI-friendly; works locally
# on a developer machine that has GitHub Desktop installed.

[CmdletBinding()]
param(
    [string]$Target = 'x86_64-pc-windows-msvc',
    [string]$ExpectedGhdVersion # optional, e.g. '3.5.12' — pins the assertion
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Fail([string]$msg) {
    Write-Host "::error::$msg" -ForegroundColor Red
    throw $msg
}

function Section([string]$msg) {
    Write-Host ""
    Write-Host "==> $msg" -ForegroundColor Cyan
}

# ----------------------------------------------------------------------
# 0. Sanity: GitHub Desktop is installed.
# ----------------------------------------------------------------------
Section "Verifying GitHub Desktop install layout"

$ghdRoot   = Join-Path $env:LOCALAPPDATA 'GitHubDesktop'
$launcher  = Join-Path $ghdRoot 'bin\github'
if (-not (Test-Path $launcher)) {
    Fail "GitHub Desktop launcher not found at $launcher. Run install-github-desktop.ps1 first."
}
Write-Host "Launcher: $launcher"

# ----------------------------------------------------------------------
# 1. Build the shim (release).
# ----------------------------------------------------------------------
Section "Building git-shim --release --target $Target"
& cargo build --release --target $Target
if ($LASTEXITCODE -ne 0) { Fail "cargo build failed" }

$shim = Join-Path $PSScriptRoot "..\target\$Target\release\git-shim.exe"
$shim = (Resolve-Path $shim).Path
if (-not (Test-Path $shim)) { Fail "Built shim not found at $shim" }
Write-Host "Shim:     $shim"

# ----------------------------------------------------------------------
# 2. Resolve via env var (default code path).
# ----------------------------------------------------------------------
Section "Case 1: resolve via %LOCALAPPDATA% env var"

$env:GIT_SHIM_PRINT_RESOLVED = '1'
try {
    $resolvedEnv = & $shim
    if ($LASTEXITCODE -ne 0) { Fail "shim exit $LASTEXITCODE (env case); stderr above" }
} finally {
    Remove-Item Env:GIT_SHIM_PRINT_RESOLVED
}
$resolvedEnv = ($resolvedEnv | Out-String).Trim()
Write-Host "Resolved (env):      $resolvedEnv"

# --- Assertions on the resolved path ----------------------------------
$expectedPrefix = "$ghdRoot\"
if (-not $resolvedEnv.StartsWith($expectedPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    Fail "Resolved path does not live under $expectedPrefix"
}
if ($resolvedEnv -notmatch '\\app-[0-9][^\\]*\\') {
    Fail "Resolved path is missing the expected '\app-<version>\' segment: $resolvedEnv"
}
if (-not $resolvedEnv.EndsWith('\resources\app\git\cmd\git.exe', [StringComparison]::OrdinalIgnoreCase)) {
    Fail "Resolved path has wrong suffix: $resolvedEnv"
}
if (-not (Test-Path $resolvedEnv)) {
    Fail "Resolved path does not exist on disk: $resolvedEnv"
}

# Optional: pin to a specific GHD version.
if ($ExpectedGhdVersion) {
    $needle = "\app-$ExpectedGhdVersion\"
    if ($resolvedEnv -notlike "*$needle*") {
        Fail "Resolved path does not contain expected version segment '$needle': $resolvedEnv"
    }
}

# --- Negative assertion: must NOT be the runner's system git ----------
$systemGit = $null
$cmd = Get-Command git.exe -ErrorAction SilentlyContinue
if ($cmd) { $systemGit = $cmd.Source }
if ($systemGit) {
    Write-Host "System git on PATH:  $systemGit"
    if ($resolvedEnv -ieq $systemGit) {
        Fail "Shim resolved to the runner's system git, not GitHub Desktop's bundled git."
    }
} else {
    Write-Host "No system git on PATH (acceptable — confirms shim is not delegating to PATH lookup)."
}

# ----------------------------------------------------------------------
# 3. Resolve via Known Folders fallback (env var scrubbed).
# ----------------------------------------------------------------------
Section "Case 2: resolve via SHGetKnownFolderPath fallback (LOCALAPPDATA cleared)"

# Spawn a fresh PowerShell child with LOCALAPPDATA removed. The shim's
# fallback (`SHGetKnownFolderPath(FOLDERID_LocalAppData)`) must succeed.
$psArgs = @(
    '-NoProfile', '-NonInteractive', '-Command',
    "Remove-Item Env:LOCALAPPDATA -ErrorAction SilentlyContinue; " +
    "`$env:GIT_SHIM_PRINT_RESOLVED='1'; " +
    "& '$shim'; exit `$LASTEXITCODE"
)
$resolvedFallback = & powershell.exe @psArgs
if ($LASTEXITCODE -ne 0) { Fail "shim exit $LASTEXITCODE (fallback case); stderr above" }
$resolvedFallback = ($resolvedFallback | Out-String).Trim()
Write-Host "Resolved (fallback): $resolvedFallback"

if ($resolvedFallback -ine $resolvedEnv) {
    Fail "Env-path and Known-Folders-fallback resolved to different paths:`n  env:      $resolvedEnv`n  fallback: $resolvedFallback"
}

# ----------------------------------------------------------------------
# 4. Forwarded `git --version` runs against GHD's bundled git.
# ----------------------------------------------------------------------
Section "Case 3: forwarded execution — shim --version"

$forwarded = & $shim --version
if ($LASTEXITCODE -ne 0) { Fail "shim --version exit $LASTEXITCODE; output: $forwarded" }
$forwarded = ($forwarded | Out-String).Trim()
Write-Host "shim --version output: $forwarded"
if ($forwarded -notmatch '^git version \d') {
    Fail "shim --version did not return a git version banner. Got: $forwarded"
}

# Compare against the bundled git directly to prove parity.
$direct = & $resolvedEnv --version
if ($LASTEXITCODE -ne 0) { Fail "direct git --version exit $LASTEXITCODE" }
$direct = ($direct | Out-String).Trim()
Write-Host "direct bundled git:    $direct"
if ($forwarded -ne $direct) {
    Fail "Shim's --version output ($forwarded) differs from bundled git's ($direct)"
}

# If a system git exists and reports a DIFFERENT version, that further
# proves we're not accidentally calling it. (If versions happen to match
# we can't make the negative assertion, but the path check above already
# rules out the system git.)
if ($systemGit) {
    $sysVer = & $systemGit --version
    if ($LASTEXITCODE -eq 0) {
        Write-Host "system git --version:  $(($sysVer | Out-String).Trim())"
    }
}

Write-Host ""
Write-Host "OK: git-shim e2e checks passed." -ForegroundColor Green
