# Pre-seed the Tauri Windows bundler tools (WiX) so `pnpm tauri build`
# does NOT need to download them from GitHub at build time.
#
# Tauri skips the download entirely when %LOCALAPPDATA%\tauri\WixTools314
# already exists with the required binaries (see tauri-bundler msi/mod.rs).
# This matters because the GitHub release-assets CDN is often unreachable
# from restricted networks, causing `tauri build` to time out.
#
# Usage:  pwsh scripts/bundle-tools.ps1   (or: pnpm bundle:tools)

$ErrorActionPreference = "Stop"

$WIX_RELEASE = "wix3141rtm"
$WIX_ZIP    = "wix314-binaries.zip"
$WIX_BASE_URL = "https://github.com/wixtoolset/wix3/releases/download/$WIX_RELEASE/$WIX_ZIP"

# Candidate download URLs: GitHub mirrors first (reachable on restricted
# networks), then the direct GitHub URL as a last resort.
$WIX_URLS = @(
  "https://mirror.ghproxy.com/$WIX_BASE_URL",
  "https://ghproxy.net/$WIX_BASE_URL",
  "https://gh.api.99988866.xyz/$WIX_BASE_URL",
  $WIX_BASE_URL
)

$WIX_REQUIRED_FILES = @(
  "candle.exe", "candle.exe.config", "darice.cub", "light.exe",
  "light.exe.config", "wconsole.dll", "winterop.dll", "wix.dll",
  "WixUIExtension.dll", "WixUtilExtension.dll"
)

$tauriTools = Join-Path $env:LOCALAPPDATA "tauri"
$wixPath    = Join-Path $tauriTools "WixTools314"

function Write-Step($msg) { Write-Host "[bundle-tools] $msg" -ForegroundColor Cyan }
function Write-Fail($msg) {
  Write-Host "[bundle-tools] ERROR: $msg" -ForegroundColor Red
  exit 1
}

# Already present?
if (Test-Path $wixPath) {
  $missing = $WIX_REQUIRED_FILES | Where-Object { -not (Test-Path (Join-Path $wixPath $_)) }
  if ($missing.Count -eq 0) {
    Write-Step "WixTools314 already present and complete — nothing to do."
    exit 0
  } else {
    Write-Step "WixTools314 exists but missing: $($missing -join ', '). Re-downloading."
    Remove-Item $wixPath -Recurse -Force
  }
}

New-Item -ItemType Directory -Force -Path $wixPath | Out-Null

$tmpZip = Join-Path $env:TEMP "wix314-binaries.zip"
$extractDir = Join-Path $env:TEMP "wix314-extract"

if (Test-Path $extractDir) { Remove-Item $extractDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $extractDir | Out-Null

$downloaded = $false
foreach ($url in $WIX_URLS) {
  Write-Step "Trying $url"
  try {
    Invoke-WebRequest -Uri $url -OutFile $tmpZip -TimeoutSec 300 -ErrorAction Stop
    if ((Get-Item $tmpZip).Length -gt 0) { $downloaded = $true; break }
  } catch {
    Write-Host "  failed: $_" -ForegroundColor Yellow
  }
}
if (-not $downloaded) {
  $manual = "Could not download $WIX_ZIP from any source (network/CDN unreachable).`n" +
    "Manual fix:`n" +
    "  1. Download $WIX_ZIP in a browser (e.g. from $WIX_BASE_URL or a GitHub mirror).`n" +
    "  2. Extract its contents into: $wixPath`n" +
    "  3. Ensure these files exist there: " + ($WIX_REQUIRED_FILES -join ', ') + ".`n" +
    "Then re-run pnpm tauri build."
  Write-Fail $manual
}

Write-Step "Extracting…"
Expand-Archive -Path $tmpZip -DestinationPath $extractDir -Force

# The zip usually nests files under a `wix314-binaries\` directory.
$inner = Join-Path $extractDir "wix314-binaries"
if (-not (Test-Path (Join-Path $inner "candle.exe"))) { $inner = $extractDir }

Copy-Item -Path (Join-Path $inner "*") -Destination $wixPath -Recurse -Force

Remove-Item $tmpZip -Force -ErrorAction SilentlyContinue
Remove-Item $extractDir -Recurse -Force -ErrorAction SilentlyContinue

$missing = $WIX_REQUIRED_FILES | Where-Object { -not (Test-Path (Join-Path $wixPath $_)) }
if ($missing.Count -gt 0) {
  Write-Fail "Extracted WiX is missing required files: $($missing -join ', ')"
}

Write-Step "WixTools314 ready at: $wixPath"
Write-Step "You can now run: pnpm tauri build"

