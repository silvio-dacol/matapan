# Net-Worth App

An app to track the net worth of an individual over the biggest belongings and monetary assets.

## MVP

Just replicate what you have already done on the Excel dashboard but make it a little more beautiful and automated.

## Backend CLI

This repo includes a small Rust CLI that reads monthly JSON snapshots from `database/` (all `*.json` files except `template.json`), aggregates them, and writes an `output/dashboard.json` with per‑category totals and net‑worth over time.

The CLI now supports a global `settings.json` configuration file that provides default values for normalization settings, base currency, and category mappings.

### Run

- Build: `cargo build`
- Execute (defaults shown):
  - `cargo run -- --input database --output output/dashboard.json --pretty --latest-only`
  - `cargo run -- --input database --output output/dashboard.json --settings settings.json --pretty`
  - Omit `--latest-only` to aggregate the full time‑series.
  - Use `--settings` to specify a configuration file with default values.

### Configuration

The CLI supports an optional `settings.json` file that provides global defaults and configuration:

```json
{
  "base_currency": "EUR",
  "normalize_to_hicp": "yes",
  "normalize_to_ecli": "yes",
  "hicp": {
    "base_year": "2024",
    "base_month": "08",
    "base_hicp": 126.72
  },
  "ecli": {
    "cost_of_living_index_weight": 0.25,
    "rent_index_weight": 0.4,
    "groceries_index_weight": 0.35,
    "restaurant_price_index_weight": 0.0,
    "local_purchasing_power_index_weight": 0.0
  },
  "categories": {
    "assets": ["liquidity", "investments", "personal", "retirement"],
    "liabilities": ["loans", "credit_card_debt"]
  }
}
```

These settings provide defaults that can be overridden by individual snapshot files.

### CLI Options

```
USAGE:
    cargo run -- [OPTIONS]

OPTIONS:
    -i, --input <PATH>       Input folder containing JSON snapshots [default: input]
    -o, --output <PATH>      Output file for aggregated dashboard JSON [default: output/dashboard.json]
    -s, --settings <PATH>    Settings file for default configuration [optional]
        --latest-only        Only process the latest dated file
        --pretty             Pretty print JSON output [default: true]
    -h, --help              Print help information
```

**Examples:**

```bash
# Process all files with settings
cargo run -- --input database --output output/dashboard.json --settings settings.json --pretty

# Process only latest file without settings
cargo run -- --input database --output output/latest.json --latest-only

# Process all files, compact output
cargo run -- --input database --output output/compact.json --no-pretty
```

### Input format

Use `database/template.json` as a guide. Each snapshot file should contain:

**Required fields:**

- `metadata.date` in `YYYY-MM-DD`
- `net_worth_entries[]` with `name`, `type` (`liquidity|investments|personal|pension|liabilities`), `currency`, and `balance`

**Optional fields (can use defaults from settings.json):**

- `metadata.base_currency` (e.g., `EUR`) - defaults from settings
- `fx_rates` as a map: `CURRENCY -> units of base currency per 1 unit of CURRENCY` (with base `EUR`, example: `"SEK": 0.0875`). If an entry's currency equals the base, the rate is `1.0`.
- Normalization flags: `metadata.adjust_to_inflation` and `metadata.normalize_to_new_york_ecli` (both `"yes"|"no"`)
- `metadata.hicp` with `base_year`, `base_month`, `base_hicp` - can use settings defaults
- `metadata.ecli_weight` with weight configurations - can use settings defaults
- `inflation.ecli_basic` with current city indices
- `inflation.current_hicp` for the snapshot date

**File structure (new format):**

```json
{
  "version": 1,
  "metadata": {
    "date": "2024-10-10",
    "base_currency": "EUR",
    "adjust_to_inflation": "yes",
    "normalize_to_new_york_ecli": "yes",
    "hicp": {
      "base_year": "2024",
      "base_month": "08",
      "base_hicp": 126.72
    },
    "ecli_weight": {
      "rent_index_weight": 0.4,
      "groceries_index_weight": 0.35,
      "cost_of_living_index_weight": 0.25
    }
  },
  "fx_rates": {
    "EUR": 1.0,
    "SEK": 0.087188
  },
  "inflation": {
    "ecli_basic": {
      "rent_index": 7.7,
      "groceries_index": 35.7,
      "cost_of_living_index": 31.8
    },
    "current_hicp": 126.72
  },
  "net_worth_entries": [...]
}
```

**Backward compatibility:** The CLI also supports the legacy format where `fx_rates`, `ecli`, and `hicp` were nested inside `metadata`.

### Output

The CLI writes `output/dashboard.json` containing per-date snapshots. Each snapshot includes the original nominal values plus up to **three distinct adjustment views** based on configuration:

1. `inflation_adjusted` – HICP deflation only (removes general inflation over time)
2. `new_york_normalized` – Cost-of-living normalization only (shows New York purchasing power equivalent)
3. `real_purchasing_power` – Combined: `Money × Deflator / ECLI_norm` (inflation deflated AND geographically normalized)

Only the views whose required data & flags are present (either in files or settings) are included. The adjustments use the settings as defaults when data is missing from individual files. Example (all three present):

```
{
  "date": "2025-09-30",
  "base_currency": "EUR",
  "breakdown": { "cash": 120073.0, "investments": 52864.0, "personal": 773.0, "pension": 38976.0, "liabilities": 2.0 },
  "totals": { "assets": 212685.0, "liabilities": 2.0, "net_worth": 212683.0 },
  "inflation_adjusted": {
    "scale": 0.966,
    "deflator": 0.966,
    "breakdown": { "cash": 116986.5, ... },
    "totals": { "assets": 205,000.0, ... },
    "notes": "Inflation-only deflation using HICP"
  },
  "new_york_normalized": {
    "scale": 2.008,               // 1 / ECLI_norm
    "ecli_norm": 0.498,
    "breakdown": { "cash": 241000.0, ... },
    "totals": { "assets": 420,000.0, ... },
    "notes": "Cost-of-living normalization to New York"
  },
  "real_purchasing_power": {
    "scale": 1.94,                // deflator / ecli_norm
    "deflator": 0.966,
    "ecli_norm": 0.498,
    "breakdown": { "cash": 233000.0, ... },
    "totals": { "assets": 405,000.0, ... },
    "notes": "Combined inflation deflation and New York cost-of-living normalization"
  },
  "warnings": []
}
```

This file can be consumed directly by a frontend dashboard for charts (nominal vs real purchasing power vs location-normalized).

## Real Money Normalization Model

Goal: Convert nominal money into **real purchasing power**, adjusted for **inflation over time** and **cost of living across cities/countries**.

The system supports flexible configuration through `settings.json` which provides defaults for normalization parameters. Individual snapshot files can override these settings as needed.

### Base Assumptions

- **Base currency:** EUR
- **Inflation index:** Euro Area HICP (monthly, Eurostat)
- **Cost of living index:** ECLI (Essential Cost of Living Index) from Numbeo
- **Purpose:** Compare income or net worth over time and across locations

### Essential Cost of Living Index (ECLI)

ECLI represents the essential cost of living in a city and the reference city is New York. So these are the translation in the equivalent amount of money that would be needed in New York to maintain the same standard of living.

These data are found at: https://www.numbeo.com/cost-of-living/rankings.jsp

Remember to change the year to the correct one.

```
ECLI = 0.40·RentIndex + 0.35·GroceriesIndex + 0.25·CostOfLivingIndex
```

Normalize it:

```
ECLI_norm = ECLI / 100
```

### Inflation Adjustment (Deflator)

Choose a **base month** (e.g. Aug 2024 = 126.72).

The data comes from: https://ec.europa.eu/eurostat/databrowser/view/prc_hicp_midx__custom_18474914/default/table

```
Deflator(t) = HICP(base_month) / HICP(t)
```

### Final Normalization Formulas

Let `Money_EUR(t)` be the nominal amount converted to the base currency.

1. Inflation-only (HICP deflation):

```
InflationAdjusted(t) = Money_EUR(t) × Deflator(t)
```

2. New York cost-of-living normalization only:

```
NYNormalized(t) = Money_EUR(t) / ECLI_norm(city)
```

3. Combined real purchasing power:

```
RealMoney(t) = Money_EUR(t) × Deflator(t) / ECLI_norm(city)
```

Where:

- `Deflator(t) = HICP(base_month) / HICP(t)`
- `ECLI_norm(city) = (0.40·Rent + 0.35·Groceries + 0.25·CostOfLiving) / 100`

### Interpretation

- If RealMoney increases → **you are actually richer**
- Works globally, even if you move country
- Keeps money comparable over time
- Useful for salary, savings, FIRE tracking, geo-arbitrage analysis
- Configuration through `settings.json` allows consistent normalization across all snapshots
- Individual files can override settings for specific periods or locations

### Example (Gothenburg → Shanghai)

```
Money_EUR = 2920
Deflator = 0.966
ECLI(Gothenburg) = 49.8  → ECLI_norm = 0.498
```

Inflation-only (Gothenburg):

```
InflationAdjusted = 2920 × 0.966 = 2822 EUR
```

NY normalization only (Gothenburg):

```
NYNormalized = 2920 / 0.498 = 5863 EUR
```

Combined real purchasing power (Gothenburg):

```
RealMoney = 2920 × 0.966 / 0.498 = 5668 EUR
```

Combined real purchasing power (Shanghai, ECLI_norm = 0.36):

```
RealMoney = 2920 × 0.966 / 0.36 = 7832 EUR
```

Moving could increases purchasing power!
