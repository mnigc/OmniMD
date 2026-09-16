# Build the app and installers with the updater signing key configured.
#
# `bundle.createUpdaterArtifacts` is enabled in tauri.conf.json, so `tauri
# build` aborts unless TAURI_SIGNING_PRIVATE_KEY is set (it produces the
# `.sig` files and the `latest.json` payload used by the in-app updater).
# This script points that variable at the locally generated key so the plain
# `pnpm tauri build` workflow keeps working.
#
# Usage:  pwsh scripts/build-signed.ps1   (or: pnpm build:app)

$ErrorActionPreference = "Stop"

$keyPath = if ($env:TAURI_SIGNING_PRIVATE_KEY) {
  $env:TAURI_SIGNING_PRIVATE_KEY
} else {
  Join-Path $env:USERPROFILE ".tauri\omnid.key"
}

if (-not (Test-Path -LiteralPath $keyPath)) {
  Write-Host "[build] Signing key not found at $keyPath" -ForegroundColor Red
  Write-Host "[build] Generate one with:" -ForegroundColor Yellow
  Write-Host "          pnpm tauri signer generate -w `"$keyPath`"" -ForegroundColor Yellow
  exit 1
}

$env:TAURI_SIGNING_PRIVATE_KEY = $keyPath
# The generated key has no password; set this if you regenerate one with `-p`.
if (-not $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
}

Write-Host "[build] Signing with: $keyPath" -ForegroundColor Cyan
pnpm tauri build
exit $LASTEXITCODE
