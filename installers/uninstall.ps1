#!/usr/bin/env pwsh

$ErrorActionPreference = "SilentlyContinue"

Write-Host ""
Write-Host "  BeforePaste Uninstaller" -ForegroundColor Cyan
Write-Host ""

Write-Host "  [~] Removing scheduled task..." -ForegroundColor Yellow
Unregister-ScheduledTask -TaskName "BeforePaste" -Confirm:$false 2>$null

Write-Host "  [~] Removing startup shortcut..." -ForegroundColor Yellow
$startup = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Startup"
Remove-Item "$startup\BeforePaste.lnk" -Force 2>$null
Remove-Item "$startup\BeforePaste.vbs" -Force 2>$null

Write-Host "  [~] Removing binary..." -ForegroundColor Yellow
Remove-Item "$env:ProgramFiles\BeforePaste" -Recurse -Force 2>$null

Write-Host "  [~] Removing config..." -ForegroundColor Yellow
Remove-Item "$env:APPDATA\beforepaste" -Recurse -Force 2>$null

$path = [Environment]::GetEnvironmentVariable("Path", "Machine")
$newPath = ($path.Split(';') | Where-Object { $_ -ne "$env:ProgramFiles\BeforePaste" }) -join ';'
[Environment]::SetEnvironmentVariable("Path", $newPath, "Machine")

Write-Host ""
Write-Host "  [OK] BeforePaste has been removed." -ForegroundColor Green
Write-Host ""
