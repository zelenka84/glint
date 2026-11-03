# Builds the closed-beta installer "Glint Beta" for invited testers and puts it
# in dist-beta/. Same rules as the local build (offline names without a
# Microsoft check), so share it with testers directly, not as a public download.
#
#   powershell -ExecutionPolicy Bypass -File scripts\build-beta.ps1

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

Push-Location (Join-Path $repo "apps\desktop")
try {
    if (-not (Test-Path node_modules)) { npm install; if ($LASTEXITCODE) { throw "npm install failed" } }
    npx tauri build --config src-tauri/tauri.beta.conf.json --features beta
    if ($LASTEXITCODE) { throw "Build failed" }
    $version = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
} finally {
    Pop-Location
}

$installer = Get-ChildItem (Join-Path $repo "target\release\bundle\nsis") -Filter "Glint Beta_*-setup.exe" |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $installer) { throw "No installer was produced" }

$out = Join-Path $repo "dist-beta"
New-Item -ItemType Directory -Force $out | Out-Null
$dest = Join-Path $out "Glint-Beta-$version-setup.exe"
Copy-Item $installer.FullName $dest -Force
Write-Host "Beta installer: $dest"
