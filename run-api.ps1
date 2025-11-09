# Net Worth API Server Launcher
# Simple PowerShell script to run the API server with sensible defaults

param(
    [string]$DashboardPath = "dashboard/dashboard.json",
    [string]$DatabaseDir = "database",
    [string]$ServerHost = "127.0.0.1",
    [int]$Port = 3000
)

Write-Host "Starting Net Worth API Server..." -ForegroundColor Green
Write-Host "================================" -ForegroundColor Green
Write-Host ""

# Set environment variables
$env:DASHBOARD_PATH = $DashboardPath
$env:DATABASE_DIR = $DatabaseDir
$env:HOST = $ServerHost
$env:PORT = $Port

Write-Host "Configuration:" -ForegroundColor Cyan
Write-Host "  Dashboard: $DashboardPath"
Write-Host "  Database:  $DatabaseDir"
Write-Host "  Server:    http://${ServerHost}:$Port"
Write-Host ""

# Check if dashboard file exists
if (-not (Test-Path $DashboardPath)) {
    Write-Host "Warning: Dashboard file not found at '$DashboardPath'" -ForegroundColor Yellow
    Write-Host "You may need to run the CLI first to generate it:" -ForegroundColor Yellow
    Write-Host "  cargo run -- --input database --output dashboard.json --settings settings.json" -ForegroundColor Yellow
    Write-Host ""
}

# Run the server
Write-Host "Starting server..." -ForegroundColor Green
Write-Host ""
cargo run --bin server

# If we get here, the server has stopped
Write-Host ""
Write-Host "Server stopped." -ForegroundColor Yellow
