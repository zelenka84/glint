# Builds a Windows installer for one release channel and puts it in dist\<channel>\.
#
#   powershell -ExecutionPolicy Bypass -File scripts\build.ps1 -Channel dev
#   powershell -ExecutionPolicy Bypass -File scripts\build.ps1 -Channel beta
#   powershell -ExecutionPolicy Bypass -File scripts\build.ps1 -Channel public
#
# dev     "Glint Dev"  for developers: experimental features on, offline names
#                      without a Microsoft check. Not for testers or the public.
# beta    "Glint Beta" for invited testers: no experiments, offline names without
#                      a Microsoft check. Share directly, not as a public download.
# public  "Glint"      for everyone: offline names and Ely.by need a Microsoft
#                      account that owns Minecraft.
#
# Each channel installs as its own app, so all three can sit side by side.
# Instances, worlds and players are shared (the Glint folder in AppData).

param(
    [Parameter(Mandatory)]
    [ValidateSet("dev", "beta", "public")]
    [string]$Channel
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

$product = @{ dev = "Glint Dev"; beta = "Glint Beta"; public = "Glint" }[$Channel]
$tauriArgs = switch ($Channel) {
    "dev" { @("--config", "src-tauri/tauri.dev.conf.json", "--features", "dev") }
    "beta" { @("--config", "src-tauri/tauri.beta.conf.json", "--features", "beta") }
    "public" { @("--bundles", "nsis") }
}

if ($Channel -eq "public") {
    Write-Warning ("Glint has no Microsoft sign-in yet, so a Public build cannot verify game " +
        "ownership: offline names and Ely.by stay locked and nobody can play.")
}

Push-Location (Join-Path $repo "apps\desktop")
try {
    if (-not (Test-Path node_modules)) { npm install; if ($LASTEXITCODE) { throw "npm install failed" } }
    npx tauri build @tauriArgs
    if ($LASTEXITCODE) { throw "Build failed" }
    $version = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
} finally {
    Pop-Location
}

$installer = Get-ChildItem (Join-Path $repo "target\release\bundle\nsis") -Filter "${product}_$version*-setup.exe" |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $installer) { throw "No installer was produced" }

$out = Join-Path $repo "dist\$Channel"
New-Item -ItemType Directory -Force $out | Out-Null
$dest = Join-Path $out "$($product -replace ' ', '-')-$version-setup.exe"
Copy-Item $installer.FullName $dest -Force
Write-Host "$product installer: $dest"
