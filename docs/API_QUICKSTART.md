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
curl http://localhost:3000/health
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
curl http://localhost:3000/api/dashboard
```

### Get Latest Snapshot

```powershell
curl http://localhost:3000/api/dashboard/latest
```

### Get Detailed Entries for a Date

```powershell
curl http://localhost:3000/api/snapshots/2025-09-01/entries/enriched
```

## Available Endpoints

| Endpoint                                    | Description                       |
| ------------------------------------------- | --------------------------------- |
| `GET /health`                               | Health check                      |
| `GET /api/dashboard`                        | Full dashboard with all snapshots |
| `GET /api/dashboard/latest`                 | Most recent snapshot only         |
| `GET /api/snapshots/:date/entries`          | Raw entries for a specific date   |
| `GET /api/snapshots/:date/entries/enriched` | Entries with FX conversion        |

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
