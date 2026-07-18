# Windows 배포 패키징: 릴리스 빌드 + zip.
#
# 사용 (Windows PowerShell):
#   .\scripts\package-windows.ps1
#
# 서명(선택): signtool sign /fd SHA256 /a dist\hanengw.exe ... (인증서 필요)
# 로그인 자동 시작: 아래 레지스트리 등록 (제거는 Remove-ItemProperty)
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
Write-Host "완료: dist\haneng-windows.zip"
