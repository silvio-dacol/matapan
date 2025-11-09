# Net-Worth Tracker

Track net worth over time with inflation adjustment and cost-of-living normalization across multiple currencies and locations.

## Quick Start

### Full Stack (Frontend + Backend)

```powershell
# 1. Generate dashboard data from monthly snapshots
cargo run --bin cli -- --input database --output dashboard/dashboard.json --settings settings.json --pretty

# 2. Start API server (Terminal 1)
.\run-api.ps1

# 3. Start frontend dashboard (Terminal 2)
cd frontend
npm install
npm run dev
```

Visit `http://localhost:3001` to see the dashboard.

### Backend Only

```powershell
# 1. Generate dashboard data
cargo run --bin cli -- --input database --output dashboard/dashboard.json --settings settings.json --pretty

# 2. Start API server
.\run-api.ps1

# 3. Test API
Invoke-WebRequest http://localhost:3000/health
```

## Frontend Dashboard

Modern Next.js dashboard with real-time data visualization.

### Features

- 📊 Interactive charts (pie, area, line)
- 💰 Key metrics dashboard (Net Worth, Assets, Liabilities)
- 🔄 Auto-refresh every 30 seconds
- 📱 Responsive design
- 🎨 Modern UI with shadcn/ui components

### Tech Stack

- Next.js 16 (App Router) + TypeScript
- TanStack Query for data fetching
- Recharts for visualizations
- Tailwind CSS + shadcn/ui

### Documentation

- [Frontend README](frontend/README.md) - Complete frontend guide
- [Fullstack Quick Start](docs/FULLSTACK_QUICKSTART.md) - Step-by-step setup
- [Frontend Implementation](docs/FRONTEND_IMPLEMENTATION.md) - Technical details

## CLI

Reads monthly JSON snapshots from `database/`, aggregates them with normalization, and outputs `dashboard/dashboard.json`.

### Usage

```powershell
# Process all snapshots
cargo run --bin cli -- --input database --output dashboard/dashboard.json --settings settings.json --pretty

# Process latest only
cargo run --bin cli -- --input database --output dashboard/dashboard.json --latest-only
```

### Options

- `-i, --input <PATH>` - Input folder with JSON snapshots (default: `database`)
- `-o, --output <PATH>` - Output dashboard file (default: `dashboard/dashboard.json`)
- `-s, --settings <PATH>` - Settings file for defaults (optional)
- `--latest-only` - Process only the latest snapshot
- `--pretty` - Pretty-print JSON output (default: true)

### Input Format

Each snapshot file requires:

- `metadata.date` (YYYY-MM-DD)
- `net_worth_entries[]` with `name`, `type`, `currency`, `balance`

Optional (defaults from `settings.json`):

- `metadata.base_currency` - Base currency for conversion (default: EUR)
- `fx_rates` - Currency exchange rates
- `metadata.adjust_to_inflation` - Enable HICP deflation ("yes"/"no")
- `metadata.normalize_to_new_york_ecli` - Enable cost-of-living normalization ("yes"/"no")
- `inflation.current_hicp` - Current HICP index value
- `inflation.ecli_basic` - City cost-of-living indices (rent, groceries, cost_of_living)

See `database/template.json` for complete structure.

### Output

Generates snapshots with three views:

1. **Nominal** - Raw values in base currency
2. **Inflation-adjusted** - HICP-deflated values
3. **Real purchasing power** - Combined inflation + cost-of-living normalization

## API Server

REST API built with Axum for serving dashboard data.

### Endpoints

| Method | Endpoint                       | Description                           |
| ------ | ------------------------------ | ------------------------------------- |
| GET    | `/health`                      | Health check                          |
| GET    | `/api/dashboard`               | Full time series                      |
| GET    | `/api/dashboard/latest`        | Most recent snapshot                  |
| GET    | `/api/snapshots/:date/entries` | Account details for specific date     |
| POST   | `/api/cache/invalidate`        | Refresh cache after data regeneration |

### Workflow

```powershell
# Regenerate data
cargo run --bin cli -- --input database --output dashboard/dashboard.json --settings settings.json --pretty

# Invalidate cache (no restart needed)
Invoke-WebRequest -Uri http://localhost:3000/api/cache/invalidate -Method POST
```

## Normalization Model

Converts nominal money to **real purchasing power** adjusted for inflation (HICP) and cost-of-living (ECLI).

### Formulas

**ECLI (Essential Cost of Living Index):**

```
ECLI = 0.40·RentIndex + 0.35·GroceriesIndex + 0.25·CostOfLivingIndex
ECLI_norm = ECLI / 100
```

**Deflator:**

```
Deflator(t) = HICP(base) / HICP(t)
```

**Three Views:**

1. Inflation-adjusted: `Money × Deflator`
2. NY-equivalent: `Money / ECLI_norm`
3. Real purchasing power: `Money × Deflator / ECLI_norm`

### Data Sources

- **HICP**: [Eurostat HICP Index](https://ec.europa.eu/eurostat/databrowser/view/prc_hicp_midx__custom_18474914/default/table)
- **ECLI**: [Numbeo Cost of Living](https://www.numbeo.com/cost-of-living/rankings.jsp)

## Documentation

- [Fullstack Quick Start](docs/FULLSTACK_QUICKSTART.md) - Complete setup guide
- [Frontend README](frontend/README.md) - Frontend documentation
- [Frontend Implementation](docs/FRONTEND_IMPLEMENTATION.md) - Technical details
- [API Quick Start](docs/API_QUICKSTART.md) - Backend API guide
- [Backend API Details](crates/backend_api/README.md) - API implementation
- [Auto-Refresh Guide](docs/AUTO_REFRESH_GUIDE.md) - Polling implementation
