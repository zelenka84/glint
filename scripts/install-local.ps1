# Builds Glint into a single exe, installs it for the current user and puts
# "Glint" shortcuts on the desktop and in the Start menu. Run again after
# changes to update the installed copy.
#
#   powershell -ExecutionPolicy Bypass -File scripts\install-local.ps1
#
# This is the Dev build with your usual settings (same app identifier as Public):
# experimental features on, offline names without a Microsoft check.

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $env:LOCALAPPDATA "Programs\Glint"
$exe = Join-Path $dest "Glint.exe"

$running = Get-Process -Name Glint -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }
if ($running) {
    Write-Error "Glint is running from $exe. Close it and run this script again."
}

Push-Location (Join-Path $repo "apps\desktop")
try {
    if (-not (Test-Path node_modules)) { npm install; if ($LASTEXITCODE) { throw "npm install failed" } }
    npx tauri build --no-bundle --features dev
    if ($LASTEXITCODE) { throw "Build failed" }
} finally {
    Pop-Location
}

New-Item -ItemType Directory -Force $dest | Out-Null
Copy-Item (Join-Path $repo "target\release\glint-desktop.exe") $exe -Force

$shell = New-Object -ComObject WScript.Shell
foreach ($folder in [Environment]::GetFolderPath("Desktop"), [Environment]::GetFolderPath("Programs")) {
    $link = $shell.CreateShortcut((Join-Path $folder "Glint.lnk"))
    $link.TargetPath = $exe
    $link.WorkingDirectory = $dest
    $link.IconLocation = "$exe,0"
    $link.Description = "Glint launcher"
    $link.Save()
}

Write-Host "Installed $exe"
Write-Host "Shortcuts: Desktop and Start menu"
