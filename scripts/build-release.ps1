[CmdletBinding()]
param(
    [switch]$NoBundle
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repositoryRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $repositoryRoot

# Import MSVC variables when the script is started from a normal PowerShell.
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (Test-Path -LiteralPath $vswhere) {
    $installPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($installPath) {
        $developerCommand = Join-Path $installPath 'Common7\Tools\VsDevCmd.bat'
        $developerEnvironment = & cmd.exe /d /s /c "`"$developerCommand`" -arch=x64 -host_arch=x64 >nul && set"
        foreach ($line in $developerEnvironment) {
            $separator = $line.IndexOf('=')
            if ($separator -gt 0) {
                Set-Item -Path "Env:$($line.Substring(0, $separator))" -Value $line.Substring($separator + 1)
            }
        }
    }
}

$defaultKeyPath = Join-Path $env:USERPROFILE '.tauri\akex.key'
$defaultPasswordPath = Join-Path $env:USERPROFILE '.tauri\akex.key.password'
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    if (-not (Test-Path -LiteralPath $defaultKeyPath)) {
        throw "Updater-Signierschlüssel fehlt: $defaultKeyPath"
    }
    # Tauri accepts either the private-key contents or its filesystem path in
    # TAURI_SIGNING_PRIVATE_KEY. Keeping the path here avoids reading or
    # printing the secret in the build script.
    $env:TAURI_SIGNING_PRIVATE_KEY = $defaultKeyPath
}

if (-not (Test-Path Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD)) {
    if (-not (Test-Path -LiteralPath $defaultPasswordPath)) {
        throw "Updater-Schlüsselpasswort fehlt: $defaultPasswordPath"
    }
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Get-Content -Raw -LiteralPath $defaultPasswordPath
}

Write-Host 'Baue Akex inklusive signierter Updater-Artefakte ...' -ForegroundColor Cyan
$arguments = @('tauri', 'build')
if ($NoBundle) { $arguments += '--no-bundle' }
& npx @arguments
if ($LASTEXITCODE -ne 0) { throw "Tauri-Build fehlgeschlagen (Exitcode $LASTEXITCODE)." }

Write-Host 'Build erfolgreich: src-tauri\target\release\bundle' -ForegroundColor Green
