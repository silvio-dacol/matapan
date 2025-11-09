# Net Worth Dashboard - Frontend Development Server
# This script starts the Next.js development server

Write-Host "Starting Next.js Frontend Development Server..." -ForegroundColor Green
Write-Host ""

# Check if we're in the frontend directory
if (Test-Path "package.json") {
    Write-Host "✓ Found package.json" -ForegroundColor Green
} else {
    Write-Host "✗ package.json not found. Are you in the frontend directory?" -ForegroundColor Red
    Write-Host "Run: cd frontend" -ForegroundColor Yellow
    exit 1
}

# Check if node_modules exists
if (!(Test-Path "node_modules")) {
    Write-Host "node_modules not found. Installing dependencies..." -ForegroundColor Yellow
    npm install
}

# Check if backend is running
try {
    $response = Invoke-WebRequest -Uri "http://localhost:3000/api/dashboard" -Method GET -TimeoutSec 2 -ErrorAction Stop
    Write-Host "✓ Backend API is running on http://localhost:3000" -ForegroundColor Green
} catch {
    Write-Host "⚠ Warning: Backend API is not responding on http://localhost:3000" -ForegroundColor Yellow
    Write-Host "  Make sure to start the backend server first:" -ForegroundColor Yellow
    Write-Host "  Run: .\run-api.ps1 (from project root)" -ForegroundColor Yellow
    Write-Host ""
}

Write-Host ""
Write-Host "Starting frontend on http://localhost:3000..." -ForegroundColor Cyan
Write-Host ""
Write-Host "Press Ctrl+C to stop the server" -ForegroundColor Gray
Write-Host ""

# Start the dev server
npm run dev
