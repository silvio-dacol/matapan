#requires -Version 5.1
<#
.SYNOPSIS
    Starts the Matapan backend and frontend for local testing.

.DESCRIPTION
    One-command launcher for the local Matapan stack. Safe to rerun; setup
    steps are skipped when they are already done:

      1. Verifies Python 3.12+ and Node.js 20.19+/22.12+ are available.
      2. Creates backend\.venv and installs backend dependencies on first run.
      3. Applies pending Alembic migrations (existing data is never reset).
      4. Starts the FastAPI backend on http://127.0.0.1:8000.
      5. Runs "npm ci" for frontend\node_modules on first run.
      6. Starts the Vite frontend on http://127.0.0.1:5173.
      7. Opens http://127.0.0.1:5173 in the default browser.

    Both servers write their logs into this console. Press Ctrl+C here to
    stop them. If a port is already taken, close the other Matapan window
    first. A MATAPAN_DATABASE_URL set before starting is inherited.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\start-dev.ps1

.EXAMPLE
    .\start-dev.ps1 -NoBrowser
#>

[CmdletBinding()]
param(
    # Do not open the default browser after both servers are ready.
    [switch]$NoBrowser
)

$ErrorActionPreference = 'Stop'

$root        = $PSScriptRoot
$backendDir  = Join-Path $root 'backend'
$frontendDir = Join-Path $root 'frontend'
$backendUrl  = 'http://127.0.0.1:8000'
$frontendUrl = 'http://127.0.0.1:5173'
$venvPython  = Join-Path $backendDir '.venv\Scripts\python.exe'

$backend  = $null
$frontend = $null

function Write-Step {
    param([string]$Message)
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Write-Fail {
    param([string]$Message)
    Write-Host "ERROR: $Message" -ForegroundColor Red
}

function Test-PortListening {
    param([int]$Port)
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $connecting = $client.ConnectAsync('127.0.0.1', $Port)
        return ($connecting.Wait(300) -and $client.Connected)
    } catch {
        return $false
    } finally {
        $client.Dispose()
    }
}

function Get-PythonCommand {
    # Finds Python 3.12+ as 'python' or 'py -3'; returns $null when absent.
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        foreach ($candidate in @(
            @{ Exe = 'python'; VersionArgs = @() },
            @{ Exe = 'py';     VersionArgs = @('-3') }
        )) {
            $command = Get-Command $candidate.Exe -ErrorAction SilentlyContinue
            if ($null -eq $command) { continue }
            $versionArgs = @($candidate.VersionArgs)
            $output = & $command.Source @versionArgs -c "import sys; print('%d.%d' % sys.version_info[:2])" 2>$null
            if ($LASTEXITCODE -ne 0 -or -not $output) { continue }
            $version = [version](($output | Select-Object -First 1).ToString().Trim())
            $supported = ($version.Major -gt 3) -or ($version.Major -eq 3 -and $version.Minor -ge 12)
            if ($supported) {
                return @{ Exe = $command.Source; VersionArgs = $versionArgs; Version = $version }
            }
        }
    } finally {
        $ErrorActionPreference = $previousPreference
    }
    return $null
}

function Get-NodeVersion {
    # Returns the installed Node.js version, or $null when absent.
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $command = Get-Command node -ErrorAction SilentlyContinue
        if ($null -eq $command) { return $null }
        $output = & $command.Source --version 2>$null
        if ($LASTEXITCODE -ne 0 -or -not $output) { return $null }
        return [version](($output | Select-Object -First 1).ToString().Trim().TrimStart('v'))
    } finally {
        $ErrorActionPreference = $previousPreference
    }
}

function Assert-SupportedNode {
    # Matches the frontend engines field: ^20.19.0 || >=22.12.0
    param([version]$Version)
    return ($Version.Major -eq 20 -and $Version.Minor -ge 19) -or
           ($Version.Major -eq 22 -and $Version.Minor -ge 12) -or
           ($Version.Major -gt 22)
}

function Test-PythonImports {
    # Reports whether the interpreter can import the given modules.
    param([string]$Interpreter, [string]$Modules)
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        & $Interpreter -c "import $Modules" > $null 2>&1
        return ($LASTEXITCODE -eq 0)
    } finally {
        $ErrorActionPreference = $previousPreference
    }
}

function Wait-ForHttp {
    # Polls a URL until it answers 200, the deadline passes, or the process exits.
    param(
        [string]$Url,
        [int]$TimeoutSeconds,
        [System.Diagnostics.Process]$Process,
        [string]$Label
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if ($Process.HasExited) {
            Write-Fail "$Label exited early with code $($Process.ExitCode); see its log above."
            return $false
        }
        try {
            $response = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 2
            if ($response.StatusCode -eq 200) { return $true }
        } catch {
        }
        Start-Sleep -Milliseconds 400
    }
    return $false
}

function Stop-ProcessTree {
    # Kills a process and its children; tolerates an already-exited process.
    param([System.Diagnostics.Process]$Process)
    if ($null -eq $Process) { return }
    try {
        if (-not $Process.HasExited) {
            $previousPreference = $ErrorActionPreference
            $ErrorActionPreference = 'Continue'
            try {
                & taskkill.exe /PID $Process.Id /T /F 2>$null | Out-Null
                $null = $Process.WaitForExit(5000)
            } finally {
                $ErrorActionPreference = $previousPreference
            }
        }
    } catch {
    }
}

try {
    Write-Host 'Matapan local launcher'
    Write-Host "Repository: $root"
    Write-Host ''

    Write-Step 'Checking prerequisites'
    $python = Get-PythonCommand
    if ($null -eq $python) {
        throw 'Python 3.12 or newer was not found. Install it from https://www.python.org/downloads/ and make sure "python" is on PATH.'
    }
    Write-Host "    Python $($python.Version) at $($python.Exe)"

    $nodeVersion = Get-NodeVersion
    if ($null -eq $nodeVersion) {
        throw 'Node.js was not found. Install Node.js 20.19+ or 22.12+ from https://nodejs.org/ and make sure "node" is on PATH.'
    }
    if (-not (Assert-SupportedNode -Version $nodeVersion)) {
        throw "Node.js $($nodeVersion) is not supported; this project needs 20.19.x or 22.12+."
    }
    Write-Host "    Node.js $($nodeVersion)"

    $npm = Get-Command npm.cmd -ErrorAction SilentlyContinue
    if ($null -eq $npm) {
        throw 'npm.cmd was not found on PATH. Reinstall Node.js so that npm is available.'
    }
    Write-Host "    npm at $($npm.Source)"
    Write-Host ''

    foreach ($port in 8000, 5173) {
        if (Test-PortListening -Port $port) {
            $owner = if ($port -eq 8000) { 'backend' } else { 'frontend' }
            throw "Port $port is already in use; close the other Matapan $owner window (or stop that process) and run this script again."
        }
    }

    if (-not (Test-Path $backendDir)) { throw "The backend directory is missing: $backendDir" }
    if (-not (Test-Path $frontendDir)) { throw "The frontend directory is missing: $frontendDir" }

    if (-not (Test-Path $venvPython)) {
        Write-Step 'Creating the backend virtual environment (first run only)'
        Push-Location $backendDir
        try {
            & $python.Exe @($python.VersionArgs) -m venv .venv
            if ($LASTEXITCODE -ne 0) { throw 'python -m venv failed; see the output above.' }
        } finally {
            Pop-Location
        }
    }
    if (-not (Test-Path $venvPython)) { throw "The virtual environment interpreter is missing: $venvPython" }

    if (Test-PythonImports -Interpreter $venvPython -Modules 'uvicorn, alembic, sqlalchemy, matapan') {
        Write-Host '==> Backend dependencies are already installed.'
    } else {
        Write-Step 'Installing backend dependencies (first run; this can take a few minutes)'
        Push-Location $backendDir
        try {
            & $venvPython -m pip install -e ".[dev]"
            if ($LASTEXITCODE -ne 0) { throw 'pip install failed; see the output above.' }
        } finally {
            Pop-Location
        }
    }

    Write-Step 'Applying database migrations (existing data is preserved)'
    Push-Location $backendDir
    try {
        & $venvPython -m alembic upgrade head
        if ($LASTEXITCODE -ne 0) { throw 'alembic upgrade head failed; see the output above.' }
    } finally {
        Pop-Location
    }

    Write-Step 'Starting the backend on http://127.0.0.1:8000'
    $backend = Start-Process -FilePath $venvPython `
        -ArgumentList '-m', 'uvicorn', 'matapan.main:app', '--host', '127.0.0.1', '--port', '8000', '--reload' `
        -WorkingDirectory $backendDir -NoNewWindow -PassThru

    if (-not (Wait-ForHttp -Url "$backendUrl/health" -TimeoutSeconds 60 -Process $backend -Label 'backend')) {
        throw 'The backend did not become healthy within 60 seconds; see its log above.'
    }
    Write-Host '    Backend is healthy.'

    if (Test-Path (Join-Path $frontendDir 'node_modules')) {
        Write-Host '==> Frontend dependencies are already installed.'
    } else {
        Write-Step 'Installing frontend dependencies (first run; this can take a few minutes)'
        Push-Location $frontendDir
        try {
            & $npm.Source ci
            if ($LASTEXITCODE -ne 0) {
                Write-Fail 'npm ci failed; package.json and package-lock.json are probably out of sync.'
                Write-Host '    Falling back to npm install, which also refreshes package-lock.json.'
                & $npm.Source install
                if ($LASTEXITCODE -ne 0) { throw 'npm install failed; see the output above.' }
            }
        } finally {
            Pop-Location
        }
    }

    Write-Step 'Starting the frontend on http://127.0.0.1:5173'
    $frontend = Start-Process -FilePath $env:ComSpec `
        -ArgumentList '/d', '/c', 'npm.cmd', 'run', 'dev' `
        -WorkingDirectory $frontendDir -NoNewWindow -PassThru

    if (-not (Wait-ForHttp -Url $frontendUrl -TimeoutSeconds 60 -Process $frontend -Label 'frontend')) {
        throw 'The frontend did not become ready within 60 seconds; see its log above.'
    }
    Write-Host '    Frontend is ready.'

    if ($NoBrowser) {
        Write-Host '    Skipping browser launch (-NoBrowser).'
    } else {
        Write-Step "Opening $frontendUrl in the default browser"
        Start-Process $frontendUrl
    }

    $dbNote = if ($env:MATAPAN_DATABASE_URL) { $env:MATAPAN_DATABASE_URL } else { 'backend\data\matapan.sqlite3' }
    Write-Host ''
    Write-Host 'Matapan is running:' -ForegroundColor Green
    Write-Host "  Frontend:   $frontendUrl"
    Write-Host "  API docs:   $backendUrl/docs"
    Write-Host "  Database:   $dbNote (migrations applied; data preserved)"
    Write-Host '  Press Ctrl+C in this window to stop both servers.'
    Write-Host ''

    while ($true) {
        if ($backend.HasExited) { throw 'The backend process stopped unexpectedly; see its log above.' }
        if ($frontend.HasExited) { throw 'The frontend process stopped unexpectedly; see its log above.' }
        Start-Sleep -Seconds 1
    }
} catch {
    Write-Fail $_.Exception.Message
    exit 1
} finally {
    Write-Step 'Stopping servers'
    Stop-ProcessTree -Process $frontend
    Stop-ProcessTree -Process $backend
    Write-Host 'Both servers are stopped.'
}
