# Windows release packaging: build + zip.
# NOTE: keep this file ASCII-only — Windows PowerShell 5.1 reads scripts
# without a BOM as ANSI and chokes on UTF-8 Korean text.
#
# Usage:
#   pwsh -File scripts\package-windows.ps1   (or .\scripts\package-windows.ps1)
#
# Signing (optional): signtool sign /fd SHA256 /a dist\hanengw.exe (needs a cert)
# Autostart at login:
#   Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" `
#     -Name haneng -Value "C:\Program Files\haneng\hanengw.exe"
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

cargo build --release -p haneng-windows -p haneng-settings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

New-Item -ItemType Directory -Force dist | Out-Null
Copy-Item target\release\hanengw.exe, target\release\haneng-settings.exe dist\
Compress-Archive -Force -Path dist\hanengw.exe, dist\haneng-settings.exe `
    -DestinationPath dist\haneng-windows.zip
Write-Host "done: dist\haneng-windows.zip"

# MSI (WiX Toolset v3 - preinstalled on GitHub windows runners).
# Installs to Program Files\haneng + login autostart + launch after install.
$candle = Get-Command candle.exe -ErrorAction SilentlyContinue
if (-not $candle) {
    $wixBin = "${env:WIX}bin"
    if (Test-Path "$wixBin\candle.exe") {
        $env:PATH = "$wixBin;$env:PATH"
        $candle = Get-Command candle.exe -ErrorAction SilentlyContinue
    }
}
if ($candle) {
    $version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"')[0].Matches.Groups[1].Value
    candle.exe -nologo -arch x64 "-dVersion=$version" "-dDist=dist" `
        -out dist\haneng.wixobj scripts\haneng.wxs
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    light.exe -nologo -sice:ICE61 dist\haneng.wixobj -out dist\haneng-windows.msi
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    Write-Host "done: dist\haneng-windows.msi (v$version)"
} else {
    Write-Host "WiX not found - skipping MSI (zip only)"
}
