# Builds a Windows installer of the "local" build for closed alpha testers:
# offline names work without a Microsoft check, as in a dev build.
# Do not publish it; public releases go through ownership verification.
#
#   powershell -ExecutionPolicy Bypass -File scripts\build-alpha.ps1
#
# Output: target\release\bundle\nsis\Glint_<version>_x64-setup.exe

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

Push-Location (Join-Path $repo "apps\desktop")
try {
    if (-not (Test-Path node_modules)) { npm install; if ($LASTEXITCODE) { throw "npm install failed" } }
    npx tauri build --features local --bundles nsis
    if ($LASTEXITCODE) { throw "Build failed" }
} finally {
    Pop-Location
}

Get-ChildItem (Join-Path $repo "target\release\bundle\nsis\*.exe") | ForEach-Object {
    Write-Host ("Installer: {0} ({1:N1} MB)" -f $_.FullName, ($_.Length / 1MB))
}
