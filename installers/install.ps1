#!/usr/bin/env pwsh
#Requires -Version 5.1

$ErrorActionPreference = "Stop"
$LogFile = "$env:TEMP\beforepaste-install.log"
$ProjectDir = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
$InstallDir = "$env:LOCALAPPDATA\Programs\beforepaste"

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Require-Cargo {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "Rust/Cargo is required. Install Rust from https://rustup.rs, then rerun this script."
    }
}

function Build-Binary {
    Push-Location $ProjectDir
    try {
        cargo build --release *>> $LogFile
    } finally {
        Pop-Location
    }

    $bin = Join-Path $ProjectDir "target\release\beforepaste.exe"
    if (-not (Test-Path $bin)) {
        throw "Build finished but $bin was not found. Check $LogFile."
    }
    return $bin
}

function Install-Binary {
    param([string]$Source)

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    $dest = Join-Path $InstallDir "beforepaste.exe"
    Copy-Item $Source $dest -Force

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($userPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
        $env:Path += ";$InstallDir"
        Write-Host "Added $InstallDir to the user PATH." -ForegroundColor Yellow
    }

    return $dest
}

try {
    Write-Host ""
    Write-Host "BeforePaste source installer" -ForegroundColor Cyan
    Write-Host "Log: $LogFile" -ForegroundColor Yellow

    Write-Step "Checking Rust toolchain"
    Require-Cargo

    Write-Step "Building BeforePaste"
    $built = Build-Binary

    Write-Step "Installing CLI binary"
    $installed = Install-Binary -Source $built
    Write-Host "Installed to $installed" -ForegroundColor Green

    Write-Step "Initializing user config"
    & $installed init

    Write-Host ""
    Write-Host "BeforePaste CLI is ready." -ForegroundColor Green
    Write-Host "Use 'beforepaste trigger' for one-shot redaction, or install the desktop app for tray-based paste protection."
}
catch {
    Write-Host ""
    Write-Host "Installation failed: $_" -ForegroundColor Red
    Write-Host "Check log: $LogFile" -ForegroundColor Yellow
    exit 1
}
