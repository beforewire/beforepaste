#!/usr/bin/env pwsh

$ErrorActionPreference = "SilentlyContinue"

Write-Host ""
Write-Host "  BeforePaste Uninstaller" -ForegroundColor Cyan
Write-Host ""

Write-Host "  [~] Removing scheduled tasks..." -ForegroundColor Yellow
Unregister-ScheduledTask -TaskName "BeforePaste" -Confirm:$false 2>$null
Unregister-ScheduledTask -TaskName "com.beforewire.beforepaste-update-check" -Confirm:$false 2>$null

Write-Host "  [~] Removing startup shortcuts..." -ForegroundColor Yellow
$startup = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Startup"
Remove-Item "$startup\BeforePaste.lnk" -Force 2>$null
Remove-Item "$startup\BeforePaste.vbs" -Force 2>$null

Write-Host "  [~] Removing binaries..." -ForegroundColor Yellow
$installDirs = @(
    "$env:LOCALAPPDATA\Programs\beforepaste",
    "$env:ProgramFiles\BeforePaste",
    "$env:ProgramFiles\beforepaste"
)
foreach ($dir in $installDirs) {
    if ($dir) { Remove-Item $dir -Recurse -Force 2>$null }
}

Write-Host "  [~] Removing config..." -ForegroundColor Yellow
$configDirs = @(
    "$env:APPDATA\beforewire\beforepaste",
    "$env:LOCALAPPDATA\beforewire\beforepaste",
    "$env:APPDATA\beforepaste"
)
foreach ($dir in $configDirs) {
    if ($dir) { Remove-Item $dir -Recurse -Force 2>$null }
}

Write-Host "  [~] Removing PATH entries..." -ForegroundColor Yellow
foreach ($scope in @("User", "Machine")) {
    $path = [Environment]::GetEnvironmentVariable("Path", $scope)
    if (-not $path) { continue }
    $newPath = ($path.Split(';') | Where-Object {
        $_ -and $_ -notin $installDirs
    }) -join ';'
    [Environment]::SetEnvironmentVariable("Path", $newPath, $scope)
}

Write-Host ""
Write-Host "  [OK] BeforePaste has been removed." -ForegroundColor Green
Write-Host ""
