# quality-gate.ps1
# Windows PowerShell entry point for the quality gate pipeline.
# Sets working directory and invokes the Node-based runner.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts/ci/quality-gate.ps1

param(
    [switch]$SkipAndroid,
    [switch]$SkipLiveNetwork
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot\..\..

Write-Host "=== Legado Tauri Quality Gate ===" -ForegroundColor Cyan
Write-Host "Project: $pwd"
Write-Host ""

# Environment
$envCheck = node scripts/ci/check-env.mjs
if ($LASTEXITCODE -ne 0) {
    Write-Host "[BLOCKED] Environment check failed" -ForegroundColor Red
    exit 1
}

# Script references
Write-Host "[1/7] Checking script references..." -ForegroundColor Yellow
node scripts/ci/check-scripts.mjs
if ($LASTEXITCODE -ne 0) {
    Write-Host "[FAIL] Script reference check failed" -ForegroundColor Red
    exit 1
}

# Frontend lint
Write-Host "[2/7] Frontend lint..." -ForegroundColor Yellow
pnpm lint
if ($LASTEXITCODE -ne 0) {
    Write-Host "[FAIL] Frontend lint failed" -ForegroundColor Red
    exit 1
}

# Frontend build
Write-Host "[3/7] Frontend build..." -ForegroundColor Yellow
pnpm build
if ($LASTEXITCODE -ne 0) {
    Write-Host "[FAIL] Frontend build failed" -ForegroundColor Red
    exit 1
}

# Rust check
Write-Host "[4/7] Cargo check reader-core..." -ForegroundColor Yellow
cargo check -p reader-core
if ($LASTEXITCODE -ne 0) {
    Write-Host "[FAIL] reader-core check failed" -ForegroundColor Red
    exit 1
}

# Rust test
Write-Host "[5/7] Cargo test reader-core..." -ForegroundColor Yellow
cargo test -p reader-core
if ($LASTEXITCODE -ne 0) {
    Write-Host "[FAIL] reader-core tests failed" -ForegroundColor Red
    exit 1
}

# Tauri check
Write-Host "[6/7] Cargo check Tauri..." -ForegroundColor Yellow
cargo check -p legado-tauri
if ($LASTEXITCODE -ne 0) {
    Write-Host "[FAIL] Tauri check failed" -ForegroundColor Red
    exit 1
}

# Command contract
Write-Host "[7/7] Command contract..." -ForegroundColor Yellow
node scripts/ci/check-command-contract.mjs
if ($LASTEXITCODE -ne 0) {
    Write-Host "[WARN] Command contract has unregistered calls" -ForegroundColor DarkYellow
}

Write-Host ""
Write-Host "=== Quality Gate PASSED ===" -ForegroundColor Green
Write-Host ""

# Optional: Android build check
if (-not $SkipAndroid) {
    Write-Host "[OPTIONAL] Checking Android SDK..." -ForegroundColor DarkGray
    if (Test-Path env:ANDROID_HOME) {
        Write-Host "Android SDK found at $env:ANDROID_HOME"
    } else {
        Write-Host "[SKIP] Android SDK not configured" -ForegroundColor DarkYellow
    }
}
