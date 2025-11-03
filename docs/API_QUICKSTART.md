# API Quick Start Guide

This guide will help you get the Net Worth API up and running in minutes.

## Prerequisites

- Rust toolchain installed (1.70 or later)
- Dashboard data generated (run the CLI first)

## Step 1: Generate Dashboard Data

If you haven't already, generate the dashboard JSON:

```powershell
cargo run -- --input database --output dashboard.json --settings settings.json --pretty
```

This creates `dashboard.json` with all your net worth data.

## Step 2: Start the API Server

### Option A: Using the convenience script (Recommended)

```powershell
.\run-api.ps1
```

The server will start on `http://127.0.0.1:3000`

### Option B: Using Cargo directly

```powershell
cargo run --bin server
```

### Option C: With custom configuration

```powershell
# Custom port
.\run-api.ps1 -Port 8080

# Or with environment variables
$env:PORT=8080; $env:HOST="0.0.0.0"; cargo run --bin server
```

## Step 3: Test the API

Open your browser or use curl/Postman to test:

### Health Check

```powershell
Invoke-WebRequest http://localhost:3000/health
```

Expected response:

```json
{
  "service": "net-worth-api",
  "status": "healthy"
}
```

### Get Full Dashboard

```powershell
Invoke-WebRequest http://localhost:3000/api/dashboard
```

### Get Latest Snapshot

```powershell
Invoke-WebRequest http://localhost:3000/api/dashboard/latest
```

### Get Detailed Entries for a Date

```powershell
Invoke-WebRequest http://localhost:3000/api/snapshots/2025-09-01/entries
```

### Refresh Cache After Regenerating Data

```powershell
# After running the CLI to regenerate dashboard.json
Invoke-WebRequest -Uri http://localhost:3000/api/cache/invalidate -Method POST
```

Expected response:

```json
{
  "status": "success",
  "message": "Cache invalidated. Fresh data will be loaded on next request."
}
```

**Why use this?** When you regenerate `dashboard.json` with the CLI, the API server caches the old data in memory. This endpoint clears the cache so the API serves fresh data **without restarting the server**.

## Available Endpoints

| Endpoint                           | Description                                  |
| ---------------------------------- | -------------------------------------------- |
| `GET /health`                      | Health check                                 |
| `GET /api/dashboard`               | Full dashboard with all snapshots            |
| `GET /api/dashboard/latest`        | Most recent snapshot only                    |
| `GET /api/snapshots/:date/entries` | Account details with FX conversion           |
| `POST /api/cache/invalidate`       | Refresh cached data (after CLI regeneration) |

## Typical Workflow

### Adding New Data and Refreshing

```powershell
# 1. Add new monthly data to database/
# Edit database/2025_10.json with your new entries

# 2. Regenerate the dashboard
cargo run --bin cli -- --input database --output dashboard.json --settings settings.json --pretty

# 3. Refresh the API cache (no restart needed!)
Invoke-WebRequest -Uri http://localhost:3000/api/cache/invalidate -Method POST

# 4. Frontend automatically gets fresh data on next request
```

### Frontend Auto-Refresh Options

**Option 1: Manual Refresh Button**

```typescript
async function refreshData() {
  await fetch("http://localhost:3000/api/cache/invalidate", { method: "POST" });
  // Then reload your dashboard data
  const data = await fetch("http://localhost:3000/api/dashboard").then((r) =>
    r.json()
  );
}
```

**Option 2: Periodic Polling**

```typescript
// Check for updates every 60 seconds
setInterval(async () => {
  const data = await fetch("http://localhost:3000/api/dashboard");
  const etag = data.headers.get("etag");
  // Compare etag with previous value, reload if changed
}, 60000);
```

**Option 3: Check on Page Load**

```typescript
// Simply fetch on every page load - cache handles efficiency
const dashboard = await fetch("http://localhost:3000/api/dashboard").then((r) =>
  r.json()
);
```

## Next Steps

### For Frontend Development

1. **Install your frontend framework** (Next.js, React, Vue, etc.)
2. **Configure API base URL** in your frontend:

   ```typescript
   const API_BASE_URL = "http://localhost:3000";
   ```

3. **Fetch dashboard data**:

   ```typescript
   const response = await fetch(`${API_BASE_URL}/api/dashboard`);
   const data = await response.json();
   ```

4. **Build your charts** using the data structure documented in the API README

### Enable CORS for Production

Edit `crates/backend_api/src/router.rs` to restrict origins:

```rust
let cors = CorsLayer::new()
    .allow_origin("https://your-domain.com".parse::<HeaderValue>().unwrap())
    .allow_methods([Method::GET])
    .allow_headers([header::CONTENT_TYPE]);
```

### Add HTTPS

For production, use a reverse proxy like nginx or Caddy:

```nginx
server {
    listen 443 ssl;
    server_name api.your-domain.com;

    location / {
        proxy_pass http://127.0.0.1:3000;
    }
}
```

## Troubleshooting

### Port Already in Use

```powershell
# Use a different port
.\run-api.ps1 -Port 8080
```

### Dashboard File Not Found

```powershell
# Generate it first
cargo run -- --input database --output dashboard.json --settings settings.json
```

### CORS Errors in Browser

The API allows all origins by default for development. For production, configure CORS in `router.rs`.

## Configuration Reference

| Environment Variable | Default                 | Description                  |
| -------------------- | ----------------------- | ---------------------------- |
| `DASHBOARD_PATH`     | `dashboard.json`        | Path to dashboard file       |
| `DATABASE_DIR`       | `database`              | Directory with monthly files |
| `HOST`               | `127.0.0.1`             | Server host                  |
| `PORT`               | `3000`                  | Server port                  |
| `RUST_LOG`           | `backend_api=debug,...` | Log level                    |

## Development Tips

### Watch Mode

Automatically restart on code changes:

```powershell
cargo install cargo-watch
cargo watch -x "run --bin server"
```

### Verbose Logging

```powershell
$env:RUST_LOG="debug"; cargo run --bin server
```

### Build for Production

```powershell
cargo build --release --bin server
.\target\release\server.exe
```

## Need Help?

- Check `crates/backend_api/README.md` for detailed API documentation
- Review the main `README.md` for overall project structure
- Open an issue on GitHub for bugs or feature requests
