#Requires -Version 5.1
<#
.SYNOPSIS
  Generates a branded build of webview-shell for one specific site.

.DESCRIPTION
  Copies this project into <OutDir>/<slug>, rebrands productName / identifier /
  binary name / icon, bakes the runtime config into the binary (via
  .cargo/config.toml + WEBVIEW_SHELL_DEFAULT_CONFIG), then builds it.

  The generated app needs no sidecar JSON: URL + features are compiled in.
  CLI flags still override at runtime.

.EXAMPLE
  ./scripts/make-app.ps1 -Name "Cashier" -Url https://cashier.example.com `
      -Icon ./logo.png -Tray -Shortcut "CmdOrCtrl+Shift+K" -UserAgent chrome

.EXAMPLE
  # generate only, build later / on CI
  ./scripts/make-app.ps1 -Name "Cashier" -Url https://cashier.example.com -SkipBuild
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Name,
    [string]$Url = "",
    [string]$Identifier = "",
    [string]$Icon = "",
    [string]$OutDir = "out",
    [string]$Version = "0.1.0",
    [switch]$Tray,
    [switch]$Autostart,
    [string]$Shortcut = "",
    [string]$UserAgent = "",
    [string[]]$Inject = @(),
    [int]$Width = 1200,
    [int]$Height = 800,
    [string]$Targets = "",
    [switch]$CloseToTray,
    [switch]$Camera,
    [switch]$Microphone,
    [switch]$ExternalInBrowser,
    [string[]]$AllowedHosts = @(),
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

# --- Chrome string used by -UserAgent chrome ---------------------------------
# Keep in sync with CHROME_UA in src-tauri/src/main.rs
$chromeUa = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36"

$slug = ($Name.ToLowerInvariant() -replace '[^a-z0-9]+', '-').Trim('-')
if (-not $slug) { throw "Name '$Name' does not produce a valid slug." }

if (-not $Identifier) { $Identifier = "com.arijumadi." + ($slug -replace '-', '') }
if ($UserAgent -eq 'chrome') { $UserAgent = $chromeUa }
if ($UserAgent -eq 'default' -or $UserAgent -eq 'webview') { $UserAgent = "" }

$out = if ([System.IO.Path]::IsPathRooted($OutDir)) { $OutDir } else { Join-Path $root $OutDir }
$target = Join-Path $out $slug

Write-Host "==> Generating $Name ($slug)" -ForegroundColor Cyan
Write-Host "    target: $target"

# --- 1. copy the project (without build output) ------------------------------
if (Test-Path $target) { Remove-Item -Recurse -Force $target }
$exclude = @("node_modules", "target", "gen", "out", ".git", ".github", "webview-shell.json")

function Copy-Project([string]$Source, [string]$Destination) {
    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
    Get-ChildItem -Path $Source -Force | Where-Object { $exclude -notcontains $_.Name } | ForEach-Object {
        $child = Join-Path $Destination $_.Name
        if ($_.PSIsContainer) {
            Copy-Project -Source $_.FullName -Destination $child
        } else {
            Copy-Item $_.FullName $child -Force
        }
    }
}

Copy-Project -Source $root -Destination $target
Write-Host "    project copied"

# --- 2. rebrand tauri.conf.json ----------------------------------------------
$confPath = Join-Path $target "src-tauri/tauri.conf.json"
$conf = Get-Content $confPath -Raw | ConvertFrom-Json
$conf.productName = $Name
$conf.mainBinaryName = $slug
$conf.identifier = $Identifier
$conf.version = $Version
$conf.bundle.shortDescription = $Name
$conf.bundle.longDescription = "$Name - desktop webview"
if ($Targets) {
    # Bundler targets are per-platform (nsis/msi on Windows, deb/appimage on Linux)
    $conf.bundle.targets = @($Targets.Split(",") | ForEach-Object { $_.Trim() } | Where-Object { $_ })
}
$conf | ConvertTo-Json -Depth 20 | Set-Content $confPath -Encoding UTF8
Write-Host "    rebranded: $Identifier / $slug.exe"

# --- 3. icon ------------------------------------------------------------------
if ($Icon) {
    $iconPath = if ([System.IO.Path]::IsPathRooted($Icon)) { $Icon } else { Join-Path $root $Icon }
    if (-not (Test-Path $iconPath)) { throw "Icon not found: $iconPath" }
    $iconDir = Join-Path $target "src-tauri/icons"
    Push-Location $root
    try {
        & npx tauri icon $iconPath --output $iconDir
        if ($LASTEXITCODE -ne 0) { throw "tauri icon failed (needs a square PNG/SVG of at least 512px)" }
    } finally {
        Pop-Location
    }
    # this generator targets desktop only, so drop the mobile asset folders
    foreach ($extra in @("android", "ios")) {
        $path = Join-Path $iconDir $extra
        if (Test-Path $path) { Remove-Item -Recurse -Force $path }
    }
    Write-Host "    icons generated from $iconPath"
} else {
    Write-Host "    icon: using the default webview-shell icon (pass -Icon to change)" -ForegroundColor DarkGray
}

# --- 4. bake the runtime config into the binary ------------------------------
$defaultConfig = [ordered]@{
    url       = $Url
    title     = $Name
    width     = $Width
    height    = $Height
    tray      = [bool]$Tray
    autostart = [bool]$Autostart
    shortcut  = $Shortcut
    userAgent = $UserAgent
    inject    = @($Inject)
    closeToTray = [bool]$CloseToTray
    camera = [bool]$Camera
    microphone = [bool]$Microphone
    openExternalInBrowser = [bool]$ExternalInBrowser
    allowedHosts = @($AllowedHosts)
}
$json = $defaultConfig | ConvertTo-Json -Compress -Depth 5

# TOML basic string: escape backslash + quote
$escaped = $json.Replace('\', '\\').Replace('"', '\"')
$cargoDir = Join-Path $target ".cargo"
New-Item -ItemType Directory -Path $cargoDir -Force | Out-Null
$toml = "[env]`nWEBVIEW_SHELL_DEFAULT_CONFIG = { value = `"$escaped`" }`n"
Set-Content (Join-Path $cargoDir "config.toml") $toml -Encoding UTF8
Write-Host "    config baked in"
Write-Host "    $json" -ForegroundColor DarkGray

# --- 5. build -----------------------------------------------------------------
$exePath = Join-Path $target "src-tauri/target/release/$slug.exe"
if ($SkipBuild) {
    Write-Host "==> Done (no build). Build manually:" -ForegroundColor Green
    Write-Host "    cd `"$target`"; npm install; npm run build"
    return
}

Push-Location $target
try {
    Write-Host "==> npm install"
    & npm install
    if ($LASTEXITCODE -ne 0) { throw "npm install failed" }

    Write-Host "==> npm run build"
    & npm run build
    if ($LASTEXITCODE -ne 0) { throw "npm run build failed" }
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "==> Done: $Name" -ForegroundColor Green
foreach ($path in @(
        $exePath,
        (Join-Path $target "src-tauri/target/release/bundle/nsis"),
        (Join-Path $target "src-tauri/target/release/bundle/msi")
    )) {
    if (Test-Path $path) {
        Write-Host "    $path"
        if (Test-Path $path -PathType Container) {
            Get-ChildItem $path | ForEach-Object { Write-Host "      - $($_.Name)" -ForegroundColor DarkGray }
        }
    }
}
