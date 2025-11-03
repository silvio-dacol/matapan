# Backend API

A REST API for the Net Worth Dashboard, built with Axum and designed to serve both file-based and future PostgreSQL-backed data.

## Architecture

The API follows a repository pattern with trait-based abstraction:

- **Repository Trait**: `DashboardRepository` - abstracts data access
- **File Implementation**: `FileDashboardRepository` - reads from `dashboard.json` and `database/*.json`
- **Future**: PostgreSQL implementation can be added without changing handler logic

## API Endpoints

### Dashboard Endpoints

#### `GET /api/dashboard`

Returns the complete dashboard with all snapshots.

**Response:**

```json
{
  "generated_at": "2025-11-02T10:30:00Z",
  "base_currency": "EUR",
  "snapshots": [...],
  "latest": {...}
}
```

**Features:**

- ETags for efficient caching
- Cache-Control headers
- All values rounded to 2 decimal places

#### `GET /api/dashboard/latest`

Returns only the most recent snapshot.

**Response:**

```json
{
  "date": "2025-09-01",
  "base_currency": "EUR",
  "breakdown": {
    "cash": 50000.00,
    "investments": 150000.00,
    "personal": 25000.00,
    "pension": 75000.00,
    "liabilities": 10000.00
  },
  "totals": {
    "assets": 300000.00,
    "liabilities": 10000.00,
    "net_worth": 290000.00
  },
  "inflation_adjusted": {...},
  "real_purchasing_power": {...}
}
```

### Snapshot Detail Endpoints

#### `GET /api/snapshots/:date/entries`

Returns raw entry-level data for a specific snapshot.

**Parameters:**

- `date`: Date in `YYYY-MM-DD` format (e.g., `2025-09-01`)

**Response:**

```json
{
  "metadata": {
    "date": "2025-09-01",
    "base_currency": "EUR",
    "fx_rates": {...},
    "hicp": 118.5
  },
  "net_worth_entries": [
    {
      "name": "Cash Intesa Sanpaolo",
      "type": "cash",
      "currency": "EUR",
      "balance": 25000.00,
      "comment": ""
    },
    ...
  ]
}
```

#### `GET /api/snapshots/:date/entries/enriched`

Returns entry-level data with FX conversions applied.

**Response:**

```json
{
  "date": "2025-09-01",
  "base_currency": "EUR",
  "entries": [
    {
      "name": "Cash Intesa Sanpaolo",
      "type": "cash",
      "currency": "EUR",
      "balance": 25000.00,
      "balance_in_base": 25000.00,
      "comment": ""
    },
    {
      "name": "US Stocks",
      "type": "investments",
      "currency": "USD",
      "balance": 50000.00,
      "balance_in_base": 45000.00,
      "comment": ""
    }
  ],
  "metadata": {...}
}
```

### Health Check

#### `GET /health`

Simple health check endpoint.

**Response:**

```json
{
  "status": "healthy",
  "service": "net-worth-api"
}
```

## Running the Server

### Using Cargo

```bash
# Run with default settings (dashboard.json in current directory)
cargo run --bin server

# Run with custom paths
DASHBOARD_PATH=output/dashboard.json DATABASE_DIR=database cargo run --bin server

# Run on custom host/port
HOST=0.0.0.0 PORT=8080 cargo run --bin server
```

### Using the Convenience Script

```bash
# From the workspace root
.\run-api.ps1

# With custom port
.\run-api.ps1 -Port 8080
```

## Environment Variables

| Variable         | Default                                         | Description                                 |
| ---------------- | ----------------------------------------------- | ------------------------------------------- |
| `DASHBOARD_PATH` | `dashboard.json`                                | Path to the dashboard JSON file             |
| `DATABASE_DIR`   | `database`                                      | Directory containing monthly snapshot files |
| `HOST`           | `127.0.0.1`                                     | Server host address                         |
| `PORT`           | `3000`                                          | Server port                                 |
| `RUST_LOG`       | `backend_api=debug,tower_http=debug,axum=trace` | Logging configuration                       |

## Frontend Integration

The API is designed to support the dashboard visualizations:

### Summary Cards

Use `GET /api/dashboard/latest` or `GET /api/dashboard/summary`:

- Net Worth: `totals.net_worth`
- Total Assets: `totals.assets`
- Total Liabilities: `totals.liabilities`
- Normalized values: `real_purchasing_power.totals.*` or `inflation_adjusted.totals.*`

### Time-Series Charts

Use `GET /api/dashboard`:

- Iterate `snapshots[]` array (already chronologically sorted)
- Plot `date` on X-axis
- Plot `totals.net_worth`, `totals.assets`, etc. on Y-axis
- Add normalized series using `real_purchasing_power.totals.*`

### Category Breakdowns (Donut/Pie Charts)

Use `breakdown` from any snapshot:

```typescript
{
  cash: snapshot.breakdown.cash,
  investments: snapshot.breakdown.investments,
  personal: snapshot.breakdown.personal,
  pension: snapshot.breakdown.pension,
  liabilities: snapshot.breakdown.liabilities
}
```

### Detailed Tables

Use `GET /api/snapshots/:date/entries/enriched`:

- Shows individual accounts with FX-converted balances
- Group by `type` field for category sections
- Display both original currency and base currency amounts

### Purchasing Power Badges

Access from any snapshot with normalization:

```typescript
snapshot.real_purchasing_power?.badge;
// Example: "Relative to New York: +10.5% purchasing power"
```

## CORS Configuration

The API is configured with permissive CORS to allow frontend development:

- Allows all origins (configure for production)
- Allows all methods
- Allows all headers

For production, update `router.rs` to restrict origins:

```rust
let cors = CorsLayer::new()
    .allow_origin("https://your-frontend-domain.com".parse::<HeaderValue>().unwrap())
    .allow_methods([Method::GET])
    .allow_headers([header::CONTENT_TYPE]);
```

## Future: PostgreSQL Migration

The repository trait makes it easy to add a PostgreSQL backend:

1. Create `PostgresDashboardRepository` implementing `DashboardRepository`
2. Define migrations for tables matching the JSON structure
3. Implement the trait methods with SQL queries
4. Swap the repository in `main.rs` based on configuration

The handler logic remains unchanged—only the data source changes.

## Error Handling

All endpoints return consistent error responses:

```json
{
  "error": "Description of the error"
}
```

HTTP status codes:

- `200 OK` - Success
- `400 Bad Request` - Invalid input (e.g., malformed date)
- `404 Not Found` - Resource not found
- `500 Internal Server Error` - Server-side error

## Development

### Running Tests (Future)

```bash
cargo test -p backend_api
```

### Watching for Changes

```bash
cargo watch -x "run --bin server"
```

### Logging

Set the `RUST_LOG` environment variable to control logging:

```bash
# Detailed logging
RUST_LOG=debug cargo run --bin server

# Minimal logging
RUST_LOG=warn cargo run --bin server
```
