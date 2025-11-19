# Net-Worth Tracker

Track net worth over time with inflation adjustment and cost-of-living normalization across multiple currencies and locations.

## Quick Start

### Full Stack (Frontend + Backend)

```powershell
# 1. Generate dashboard data (choose ONE of the two methods below)

# A) Using new data_pipeline binary (direct files → dashboard):
cargo run -p data_pipeline --bin generate-dashboard -- --settings settings.json --database database --out dashboard/dashboard.json

# B) Using the CLI aggregator (still supported):
cargo run -p cli -- --input database --output dashboard/dashboard.json --settings settings.json --pretty

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
# 1. Generate dashboard data (pick one)
cargo run -p data_pipeline --bin generate-dashboard -- --settings settings.json --database database --out dashboard/dashboard.json
# OR
cargo run -p cli -- --input database --output dashboard/dashboard.json --settings settings.json --pretty

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

## Data Generation

Two supported flows produce `dashboard/dashboard.json`:

1. `data_pipeline::generate-dashboard` – Direct transformation of raw monthly files + settings.
2. `cli` – Higher-level wrapper (still available) with some extra convenience flags.

Both now emit a unified structure containing:
`metadata`, `yearly_stats[]`, and `snapshots[]` (each snapshot includes categories, cash flow, performance, and real wealth metrics already rounded).

### Yearly Stats

Per-year aggregation of cash-flow metrics:

- `year`, `months_count`, `total_income`, `total_expenses`, `total_savings`, `average_save_rate`.
  Current classification comes from `settings.json` (`positive_cash_flows`, `negative_cash_flows`).

### CLI Usage (unchanged)

```powershell
cargo run -p cli -- --input database --output dashboard/dashboard.json --settings settings.json --pretty
```

### Direct Pipeline Usage

```powershell
cargo run -p data_pipeline --bin generate-dashboard -- --settings settings.json --database database --out dashboard/dashboard.json
```

### Monthly Input File Format (raw database/\*.json)

Required fields:

```jsonc
{
  "reference_month": "2025-10", // or "month" (YYYY-MM)
  "fx_rates": { "EUR": 1.0, "SEK": 0.09104 },
  "hicp": 129.43,
  "cash-flow-entries": [
    {
      "name": "Volvo Cars Salary",
      "type": "salary",
      "currency": "SEK",
      "amount": 33852
    },
    {
      "name": "Apartment Rent",
      "type": "rent",
      "currency": "SEK",
      "amount": 8500
    }
  ],
  "net_worth_entries": [
    { "name": "SEB", "type": "cash", "currency": "SEK", "balance": 134468 },
    {
      "name": "Intesa Sanpaolo",
      "type": "investments",
      "currency": "EUR",
      "balance": 29832
    }
  ]
}
```

Accepted aliases:

- `reference_month` or `month` for the period.
- `cash-flow-entries`, `cash_flow_entries`.

### Settings File (`settings.json`)

Provides classification & base context:

```jsonc
{
  "settings_version": 1,
  "base_currency": "EUR",
  "hicp": { "base_year": 2025, "base_month": 1, "base_value": 128.5 },
  "categories": {
    "assets": ["cash", "investments", "retirement", "personal"],
    "liabilities": ["debt"],
    "positive_cash_flows": ["salary", "pension"],
    "negative_cash_flows": ["rent", "expense"]
  }
}
```

### Output Snapshot Structure (simplified)

Each snapshot:

```jsonc
{
  "month": "2025-10",
  "hicp": 129.43,
  "totals": {
    "assets": 123456.78,
    "liabilities": 2345.67,
    "net_worth": 121111.11
  },
  "by_category": { "assets": { "cash": 9999.99 }, "liabilities": {} },
  "cash_flow": {
    "income": 4000.0,
    "expenses": 2500.0,
    "net_cash_flow": 1500.0,
    "save_rate": 0.375
  },
  "performance": {
    "portfolio_nominal_return": 0.0123,
    "portfolio_real_return": 0.01,
    "twr_cumulative": 1.2345
  },
  "real_wealth": { "net_worth_real": 119000.0, "change_pct_from_prev": 0.0042 }
}
```

All numeric values are already rounded to appropriate precision (2 or 4 decimals).

### Choosing a Method

Use the pipeline binary for a lean, deterministic build. Use the CLI when you want flags like `--latest-only` (future extensions) and pretty formatting by default.

### Regeneration Cycle

```powershell
# Regenerate
cargo run -p data_pipeline --bin generate-dashboard -- --settings settings.json --database database --out dashboard/dashboard.json
# Invalidate API cache
Invoke-WebRequest -Uri http://localhost:3000/api/cache/invalidate -Method POST
```

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

- **FX Rates**: [ExchangeRate-API](https://www.x-rates.com/average/?from=USD&to=EUR&amount=1&year=2025)
- **HICP**: [Eurostat HICP Index](https://ec.europa.eu/eurostat/databrowser/view/prc_hicp_midx__custom_18474914/default/table)
- **ECLI**: [Numbeo Cost of Living](https://www.numbeo.com/cost-of-living/rankings.jsp)

## Documentation

- [Fullstack Quick Start](docs/FULLSTACK_QUICKSTART.md) - Complete setup guide
- [Frontend README](frontend/README.md) - Frontend documentation
- [Frontend Implementation](docs/FRONTEND_IMPLEMENTATION.md) - Technical details
- [API Quick Start](docs/API_QUICKSTART.md) - Backend API guide
- [Backend API Details](crates/backend_api/README.md) - API implementation
- [Auto-Refresh Guide](docs/AUTO_REFRESH_GUIDE.md) - Polling implementation
