//! FX rate fetching, caching, and lookup using freecurrencyapi.com.
//!
//! Monthly averages are computed from rates on the 1st and 28th of each month
//! and stored in `<database_dir>/fx_rates.json`.
//!
//! The `rate` field in [`FxRateEntry`] represents units of `from_currency` per
//! one unit of `to_currency` (the base currency).  For example, if the base is
//! EUR and the source currency is SEK, a `rate` of 10.4 means 10.4 SEK = 1 EUR.
//! To convert an amount from SEK to EUR: `amount_eur = amount_sek / rate`.

use anyhow::{anyhow, Context, Result};
use freecurrencyapi::Freecurrencyapi;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single monthly average FX rate entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FxRateEntry {
    /// Month in `"YYYY-MM"` format.
    pub month: String,
    /// Source currency code (e.g., `"SEK"`).
    pub from_currency: String,
    /// Base/target currency code (e.g., `"EUR"`).
    pub to_currency: String,
    /// Units of `from_currency` per one unit of `to_currency`.
    pub rate: f64,
}

// ---------------------------------------------------------------------------
// File I/O helpers
// ---------------------------------------------------------------------------

fn fx_rates_path(database_path: &Path) -> PathBuf {
    let dir = if database_path.is_dir() {
        database_path.to_path_buf()
    } else {
        database_path
            .parent()
            .unwrap_or(database_path)
            .to_path_buf()
    };
    dir.join("fx_rates.json")
}

/// Loads FX rate entries from `<database_dir>/fx_rates.json`.
///
/// Returns an empty vector if the file does not yet exist.
pub fn load_fx_rates(database_path: &Path) -> Result<Vec<FxRateEntry>> {
    let path = fx_rates_path(database_path);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Cannot read fx_rates.json at {:?}", path))?;
    let entries: Vec<FxRateEntry> = serde_json::from_str(&contents)
        .with_context(|| format!("Cannot parse fx_rates.json at {:?}", path))?;
    Ok(entries)
}

/// Saves FX rate entries to `<database_dir>/fx_rates.json`.
pub fn save_fx_rates(database_path: &Path, rates: &[FxRateEntry]) -> Result<()> {
    let path = fx_rates_path(database_path);
    let json = serde_json::to_string_pretty(rates)?;
    fs::write(&path, json)
        .with_context(|| format!("Cannot write fx_rates.json at {:?}", path))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Lookup
// ---------------------------------------------------------------------------

/// Returns the monthly average exchange rate for a currency pair.
///
/// `rate` represents units of `from_currency` per one unit of `to_currency`.
///
/// Returns `Some(1.0)` when `from_currency == to_currency`.
/// Returns `None` when the pair/month is not found in the cache.
pub fn lookup_rate(
    rates: &[FxRateEntry],
    month: &str,
    from_currency: &str,
    to_currency: &str,
) -> Option<f64> {
    if from_currency == to_currency {
        return Some(1.0);
    }
    rates
        .iter()
        .find(|e| {
            e.month == month
                && e.from_currency == from_currency
                && e.to_currency == to_currency
        })
        .map(|e| e.rate)
}

// ---------------------------------------------------------------------------
// API fetching
// ---------------------------------------------------------------------------

/// Fetches exchange rates for a single calendar date.
///
/// The API is called with `base_currency` so the response values represent
/// units of each requested currency per one unit of base.
/// For example, base=EUR, currencies="SEK" → `{"SEK": 10.5}` means 10.5 SEK = 1 EUR.
async fn fetch_rates_for_date(
    client: &Freecurrencyapi,
    base_currency: &str,
    date: &str,
    currencies_param: &str,
) -> Result<HashMap<String, f64>> {
    let response = client
        .historical(base_currency, date, currencies_param)
        .await
        .map_err(|e| anyhow!("freecurrencyapi call failed for {}: {:?}", date, e))?;

    // The library stores the raw JSON payload as a String in `response.data`.
    let data: Value = serde_json::from_str(&response.data).with_context(|| {
        format!(
            "Failed to parse freecurrencyapi response data for date {}",
            date
        )
    })?;

    // Shape: { "<date>": { "<CURRENCY>": <rate_f64>, ... } }
    let day_data = data
        .get(date)
        .ok_or_else(|| anyhow!("No data key '{}' in API response", date))?;

    let mut result = HashMap::new();
    if let Some(obj) = day_data.as_object() {
        for (currency, rate_val) in obj {
            if let Some(rate) = rate_val.as_f64() {
                result.insert(currency.clone(), rate);
            }
        }
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Sync
// ---------------------------------------------------------------------------

/// Fetches and caches monthly average FX rates for the given months.
///
/// For each month not already present in `<database_dir>/fx_rates.json` the
/// function fetches rates for the 1st and the 28th of that month, averages
/// them, and appends the new entries to the cache file.
///
/// # Arguments
/// * `api_key`           – freecurrencyapi.com API key (set `FREECURRENCYAPI_KEY`).
/// * `database_path`     – path to the database folder (or `database.json`, the
///                         parent directory is used as the cache location).
/// * `base_currency`     – the target/base currency code (e.g., `"EUR"`).
/// * `currencies`        – source currencies to ensure are cached
///                         (e.g., `&["SEK", "USD", "CNY"]`).
/// * `months`            – months in `"YYYY-MM"` format that must be available.
///
/// Returns the full (including previously cached) list of [`FxRateEntry`] items.
pub async fn sync_fx_rates(
    api_key: &str,
    database_path: &Path,
    base_currency: &str,
    currencies: &[&str],
    months: &[String],
) -> Result<Vec<FxRateEntry>> {
    let mut cached = load_fx_rates(database_path)?;

    // Build a set of (month, from_currency) pairs that are already cached so
    // we can skip API calls for months that are fully covered.
    let cached_pairs: HashSet<(String, String)> = cached
        .iter()
        .filter(|e| e.to_currency == base_currency)
        .map(|e| (e.month.clone(), e.from_currency.clone()))
        .collect();

    // Source currencies that differ from the base (no API call needed for same).
    let source_currencies: Vec<&str> = currencies
        .iter()
        .copied()
        .filter(|&c| c != base_currency)
        .collect();

    if source_currencies.is_empty() {
        return Ok(cached);
    }

    let currencies_param = source_currencies.join(",");

    // Collect months where at least one currency pair is missing.
    let missing_months: Vec<&String> = months
        .iter()
        .filter(|m| {
            source_currencies
                .iter()
                .any(|c| !cached_pairs.contains(&(m.to_string(), c.to_string())))
        })
        .collect();

    if missing_months.is_empty() {
        return Ok(cached);
    }

    let client = Freecurrencyapi::new(api_key)
        .map_err(|e| anyhow!("Failed to initialise freecurrencyapi client: {:?}", e))?;

    let mut newly_added = 0usize;

    for month in missing_months {
        let date_1st = format!("{}-01", month);
        let date_28th = format!("{}-28", month);

        let rates_1st =
            fetch_rates_for_date(&client, base_currency, &date_1st, &currencies_param).await;
        let rates_28th =
            fetch_rates_for_date(&client, base_currency, &date_28th, &currencies_param).await;

        match (rates_1st, rates_28th) {
            (Ok(r1), Ok(r28)) => {
                for &currency in &source_currencies {
                    // Skip if this specific pair is already cached.
                    if cached_pairs.contains(&(month.clone(), currency.to_string())) {
                        continue;
                    }
                    if let (Some(&v1), Some(&v28)) = (r1.get(currency), r28.get(currency)) {
                        // API returns units of `currency` per 1 base_currency.
                        // We store exactly that as-is (10.4 SEK per 1 EUR).
                        let avg_rate = (v1 + v28) / 2.0;
                        cached.push(FxRateEntry {
                            month: month.clone(),
                            from_currency: currency.to_string(),
                            to_currency: base_currency.to_string(),
                            rate: avg_rate,
                        });
                        newly_added += 1;
                    }
                }
            }
            (Err(e), _) => {
                return Err(
                    e.context(format!("Failed to fetch rates for 1st of {}", month))
                );
            }
            (_, Err(e)) => {
                return Err(
                    e.context(format!("Failed to fetch rates for 28th of {}", month))
                );
            }
        }
    }

    if newly_added > 0 {
        save_fx_rates(database_path, &cached)?;
    }

    Ok(cached)
}

// ---------------------------------------------------------------------------
// Utility: collect required months from a database JSON
// ---------------------------------------------------------------------------

/// Extracts all unique `"YYYY-MM"` months referenced by transactions and
/// positions in a database [`Value`], along with every distinct non-base
/// currency code found in those records.
///
/// Useful for determining which months and currencies to pass to
/// [`sync_fx_rates`].
pub fn collect_months_and_currencies(
    db: &Value,
    base_currency: &str,
) -> (Vec<String>, Vec<String>) {
    let mut months: HashSet<String> = HashSet::new();
    let mut currencies: HashSet<String> = HashSet::new();

    // Transactions: date "YYYY-MM-DD", currency field.
    if let Some(txns) = db.get("transactions").and_then(|v| v.as_array()) {
        for txn in txns {
            if let Some(date) = txn.get("date").and_then(|v| v.as_str()) {
                if date.len() >= 7 {
                    months.insert(date[..7].to_string());
                }
            }
            if let Some(cur) = txn.get("currency").and_then(|v| v.as_str()) {
                if cur != base_currency {
                    currencies.insert(cur.to_string());
                }
            }
        }
    }

    // Positions: as_of_date "YYYY-MM-DD", currency field.
    if let Some(positions) = db.get("positions").and_then(|v| v.as_array()) {
        for pos in positions {
            if let Some(date) = pos.get("as_of_date").and_then(|v| v.as_str()) {
                if date.len() >= 7 {
                    months.insert(date[..7].to_string());
                }
            }
            if let Some(cur) = pos.get("currency").and_then(|v| v.as_str()) {
                if cur != base_currency {
                    currencies.insert(cur.to_string());
                }
            }
        }
    }

    let mut months_vec: Vec<String> = months.into_iter().collect();
    months_vec.sort();

    let mut currencies_vec: Vec<String> = currencies.into_iter().collect();
    currencies_vec.sort();

    (months_vec, currencies_vec)
}
