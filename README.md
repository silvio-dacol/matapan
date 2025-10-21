# Net-Worth App

An app to track the net worth of an individual over the biggest belongings and monetary assets.

## MVP

Just replicate what you have already done on the Excel dashboard but make it a little more beautiful and automated.

## Backend CLI

This repo includes a small Rust CLI that reads monthly JSON snapshots from `input/` (all `*.json` files except `template.json`), aggregates them, and writes an `output/dashboard.json` with per‑category totals and net‑worth over time.

### Run

- Build: `cargo build`
- Execute (defaults shown):
  - `cargo run -- --input input --output output/dashboard.json --pretty --latest-only`
  - Omit `--latest-only` to aggregate the full time‑series.

### Input format

Use `input/template.json` as a guide. Each snapshot file should contain:

- `metadata.date` in `YYYY-MM-DD`
- `metadata.base_currency` (e.g., `EUR`)
- `fx_rates` as a map: `CURRENCY -> units of base currency per 1 unit of CURRENCY` (with base `EUR`, example: `"SEK": 0.0875`). If an entry’s currency equals the base, the rate is `1.0`.
- `net_worth_entries[]` with `name`, `type` (`liquidity|investments|personal|pension|liabilities`), `currency`, and `balance`.
- Optional normalization fields to compute “real” money (inflation + cost‑of‑living):
  - `metadata.normalize: "yes"`
  - `metadata.hicp` with `base_year`, `base_month`, `base_hicp`
  - `metadata.ECLI_weight` and `inflation.ECLI_basic`
  - `inflation.current_hicp`

### Output

The CLI writes `output/dashboard.json` similar to:

```
{
  "generated_at": "...",
  "base_currency": "EUR",
  "snapshots": [
    {
      "date": "2025-09-30",
      "base_currency": "EUR",
      "breakdown": {
        "cash": 120073.0,
        "investments": 52864.0,
        "personal": 773.0,
        "pension": 38976.0,
        "liabilities": 2.0
      },
      "totals": { "assets": 212685.0, "liabilities": 2.0, "net_worth": 212683.0 },
      "normalized": { "deflator": 0.966, "ecli_norm": 0.498, "breakdown": { "cash": 241000.0, ... } },
      "warnings": []
    }
  ],
  "latest": { ... }
}
```

This file can be consumed directly by a frontend dashboard for the charts shown in the screenshots.

## Real Money Normalization Model

Goal: Convert nominal money into **real purchasing power**, adjusted for **inflation over time** and **cost of living across cities/countries**.

### Base Assumptions

- **Base currency:** EUR
- **Inflation index:** Euro Area HICP (monthly, Eurostat)
- **Cost of living index:** ECLI (Essential Cost of Living Index) from Numbeo
- **Purpose:** Compare income or net worth over time and across locations

### Essential Cost of Living Index (ECLI)

ECLI represents the essential cost of living in a city and the reference city is New York. So these are the translation in how much money you would need in New York for the same situation.

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

### Final Normalization Formula

```
RealMoney(t) = Money_EUR(t) × Deflator(t) / ECLI_norm(city)
```

Where:

- `Money_EUR(t)` = salary, savings or net worth converted to EUR
- `Deflator(t)` = protects from inflation
- `ECLI_norm(city)` = adjusts for cost of living differences

### Interpretation

- If RealMoney increases → **you are actually richer**
- Works globally, even if you move country
- Keeps money comparable over time
- Useful for salary, savings, FIRE tracking, geo-arbitrage analysis

### Example (Gothenburg → Shanghai)

```
Money_EUR = 2920
Deflator = 0.966
ECLI(Gothenburg) = 49.8  → ECLI_norm = 0.498
```

Real purchasing power in Gothenburg:

```
RealMoney = 2920 × 0.966 / 0.498 = 5668 EUR
```

Real purchasing power in Shanghai (ECLI = 36.0 → 0.36):

```
RealMoney = 2920 × 0.966 / 0.36 = 7832 EUR
```

Moving could increases purchasing power!
