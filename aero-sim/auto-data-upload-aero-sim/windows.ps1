[CmdletBinding()]
param([string]$BinaryPath = '')

$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'This installer is for Windows. Use macos.sh or linux.sh on other systems.' }

if (-not $BinaryPath) {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw 'Install Rust from https://rustup.rs (with the MSVC build tools), or use .\windows.ps1 -BinaryPath C:\path\to\aero-archive.exe'
    }
    & cargo build --release --locked --manifest-path (Join-Path $PSScriptRoot 'Cargo.toml') --target-dir (Join-Path $PSScriptRoot 'target')
    if ($LASTEXITCODE -ne 0) { throw 'Rust release build failed.' }
    $BinaryPath = Join-Path $PSScriptRoot 'target\release\aero-archive.exe'
}
if (-not (Test-Path -LiteralPath $BinaryPath -PathType Leaf)) { throw "Binary not found: $BinaryPath" }
$BinaryPath = (Resolve-Path -LiteralPath $BinaryPath).Path
& $BinaryPath --version
if ($LASTEXITCODE -ne 0) { throw 'The binary could not run. Check that its architecture matches this PC.' }

$AppDir = Join-Path $env:LOCALAPPDATA 'Programs\Aero Archive'
$ProgramsDir = [Environment]::GetFolderPath('Programs')
New-Item -ItemType Directory -Force -Path $AppDir | Out-Null
$InstalledBinary = Join-Path $AppDir 'aero-archive.exe'
try { Copy-Item -LiteralPath $BinaryPath -Destination $InstalledBinary -Force }
catch { throw "Unable to install the executable. Close any running Aero Archive instance and retry. $($_.Exception.Message)" }

# Console subsystem executable: the Start menu shortcut opens a terminal window.
$Shell = New-Object -ComObject WScript.Shell
$Shortcut = $Shell.CreateShortcut((Join-Path $ProgramsDir 'Aero Archive.lnk'))
$Shortcut.TargetPath = $InstalledBinary
$Shortcut.WorkingDirectory = $AppDir
$Shortcut.Description = 'Tag, version and upload simulation data to GitHub'
$Shortcut.WindowStyle = 1
$Shortcut.Save()
Write-Host "Installed: $InstalledBinary"
Write-Host 'Open Aero Archive from the Start menu.'
if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Warning 'Git is missing. Run winget install --id Git.Git -e, then reopen Aero Archive for setup guidance.'
}
