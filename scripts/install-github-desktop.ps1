# Requires -Version 5.1
# Silently installs GitHub Desktop for the current user.
#
# GitHub Desktop is a Squirrel.Windows application: it installs per-user
# under %LOCALAPPDATA%\GitHubDesktop\, no admin rights required. Running
# the installer in `-NoNewWindow` non-interactive mode generally completes
# without UI on CI runners. We then wait for the launcher script to appear
# (the resolver's source of truth) before returning.

[CmdletBinding()]
param(
    # Pin a known-good release. Override with `-Version` for forward-compat.
    [string]$Version = '3.5.12',
    [int]$TimeoutSeconds = 600
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$url       = "https://github.com/desktop/desktop/releases/download/release-$Version/GitHubDesktopSetup-x64.exe"
$tempDir   = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$installer = Join-Path $tempDir "GitHubDesktopSetup-x64-$Version.exe"

Write-Host "==> Downloading GitHub Desktop $Version" -ForegroundColor Cyan
Write-Host "    $url"
Write-Host "    -> $installer"
if (-not (Test-Path $installer)) {
    Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $installer
}
$size = (Get-Item $installer).Length
Write-Host "    Downloaded $([Math]::Round($size / 1MB, 1)) MiB"

Write-Host "==> Launching installer" -ForegroundColor Cyan
# Squirrel installers self-extract and install asynchronously. We do NOT
# pass `-Wait` because the GUI auto-launches and would block CI forever.
# Instead, poll for the launcher script that the resolver depends on.
$started = Get-Date
$proc = Start-Process -FilePath $installer -PassThru
Write-Host "    PID: $($proc.Id)"

$launcher = Join-Path $env:LOCALAPPDATA 'GitHubDesktop\bin\github'
$deadline = $started.AddSeconds($TimeoutSeconds)
Write-Host "==> Waiting for launcher at $launcher" -ForegroundColor Cyan
while ((Get-Date) -lt $deadline) {
    if (Test-Path $launcher) {
        $elapsed = [int]((Get-Date) - $started).TotalSeconds
        Write-Host "    Launcher present after ${elapsed}s"
        break
    }
    Start-Sleep -Seconds 5
}
if (-not (Test-Path $launcher)) {
    Write-Host "::error::GitHub Desktop launcher did not appear within $TimeoutSeconds s"
    Get-ChildItem (Join-Path $env:LOCALAPPDATA 'GitHubDesktop') -ErrorAction SilentlyContinue |
        Format-Table -AutoSize | Out-String | Write-Host
    throw "GitHub Desktop install did not produce $launcher"
}

# Kill any auto-launched GUI processes so they cannot interfere with the
# rest of the job. The on-disk install we care about is already complete.
Get-Process -Name 'GitHubDesktop','Update','Squirrel' -ErrorAction SilentlyContinue |
    ForEach-Object {
        Write-Host "    Stopping post-install process: $($_.Name) (PID $($_.Id))"
        try { $_ | Stop-Process -Force -ErrorAction Stop } catch { }
    }

# Sanity-print the resolved app-<version> dir for the build log.
$appDirs = Get-ChildItem (Join-Path $env:LOCALAPPDATA 'GitHubDesktop') -Directory -Filter 'app-*' -ErrorAction SilentlyContinue
Write-Host "==> Installed app directories:" -ForegroundColor Cyan
$appDirs | ForEach-Object { Write-Host "    $($_.FullName)" }

$bundledGit = $null
foreach ($d in $appDirs) {
    $candidate = Join-Path $d.FullName 'resources\app\git\cmd\git.exe'
    if (Test-Path $candidate) {
        $bundledGit = $candidate
        break
    }
}
if (-not $bundledGit) { throw "No bundled git.exe found in any app-* directory" }
Write-Host "==> Bundled git: $bundledGit" -ForegroundColor Green
& $bundledGit --version

# Surface the pinned version so the e2e script can assert against it.
"GHD_VERSION=$Version"            | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append -ErrorAction SilentlyContinue
"GHD_BUNDLED_GIT=$bundledGit"     | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append -ErrorAction SilentlyContinue
