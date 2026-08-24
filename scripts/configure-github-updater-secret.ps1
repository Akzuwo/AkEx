[CmdletBinding()]
param(
    [string]$Repository = 'Akzuwo/AkEx'
)

$ErrorActionPreference = 'Stop'
$keyPath = Join-Path $env:USERPROFILE '.tauri\akex.key'
$passwordPath = Join-Path $env:USERPROFILE '.tauri\akex.key.password'
if (-not (Test-Path -LiteralPath $keyPath)) {
    throw "Privater Updater-Schlüssel nicht gefunden: $keyPath"
}
if (-not (Test-Path -LiteralPath $passwordPath)) {
    throw "Updater-Schlüsselpasswort nicht gefunden: $passwordPath"
}
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw 'GitHub CLI (gh) fehlt. Installiere sie oder hinterlege TAURI_SIGNING_PRIVATE_KEY manuell in den Repository-Secrets.'
}

Get-Content -Raw -LiteralPath $keyPath | gh secret set TAURI_SIGNING_PRIVATE_KEY --repo $Repository
if ($LASTEXITCODE -ne 0) { throw 'GitHub-Schlüssel-Secret konnte nicht gesetzt werden.' }
Get-Content -Raw -LiteralPath $passwordPath | gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo $Repository
if ($LASTEXITCODE -ne 0) { throw 'GitHub-Passwort-Secret konnte nicht gesetzt werden.' }
Write-Host "Updater-Schlüssel und -Passwort wurden für $Repository gesetzt." -ForegroundColor Green
