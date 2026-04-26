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
///
/// Entries are sorted by `(from_currency, to_currency, month)` before writing.
pub fn save_fx_rates(database_path: &Path, rates: &[FxRateEntry]) -> Result<()> {
    let path = fx_rates_path(database_path);
    let mut sorted = rates.to_vec();
    sorted.sort_by(|a, b| {
        a.from_currency
            .cmp(&b.from_currency)
            .then(a.to_currency.cmp(&b.to_currency))
            .then(a.month.cmp(&b.month))
    });
    let json = serde_json::to_string_pretty(&sorted)?;
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
// API fetching (direct reqwest â€” no third-party wrapper)
// ---------------------------------------------------------------------------

/// Fetches exchange rates for a single calendar date directly from the
/// freecurrencyapi.com v1 historical endpoint.
///
/// `base_currency` is the denominator. The returned map contains units of each
/// requested currency per one unit of `base_currency`.
/// The API may return data for the nearest available trading day when the
/// requested date falls on a weekend or public holiday; either key is accepted.
async fn fetch_rates_for_date(
    client: &reqwest::Client,
    api_key: &str,
    base_currency: &str,
    date: &str,
    currencies_param: &str,
) -> Result<HashMap<String, f64>> {
    let response = client
        .get("https://api.freecurrencyapi.com/v1/historical")
        .header("apikey", api_key)
        .query(&[
            ("base_currency", base_currency),
            ("date", date),
            ("currencies", currencies_param),
        ])
        .send()
        .await
        .with_context(|| format!("HTTP request failed for date {}", date))?;

    let status = response.status();
    let body: Value = response
        .json()
        .await
        .with_context(|| format!("Failed to parse JSON response for date {}", date))?;

    if !status.is_success() {
        let msg = body
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(anyhow!(
            "freecurrencyapi returned HTTP {} for date {}: {}",
            status,
            date,
            msg
        ));
    }

    // Response shape: { "data": { "<actual_date>": { "<CURRENCY>": <f64>, ... } } }
    // The actual date key may differ from the requested date (weekend/holiday rollover).
    let day_data = body
        .get("data")
        .and_then(|d| d.as_object())
        .and_then(|m| m.values().next())
        .ok_or_else(|| {
            anyhow!(
                "Unexpected API response structure for date {} â€” body: {}",
                date,
                body
            )
        })?;

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
/// * `api_key`           â€“ freecurrencyapi.com API key (set `FREECURRENCYAPI_KEY`).
/// * `database_path`     â€“ path to the database folder (or `database.json`, the
///                         parent directory is used as the cache location).
/// * `base_currency`     â€“ the target/base currency code (e.g., `"EUR"`).
/// * `currencies`        â€“ source currencies to ensure are cached
///                         (e.g., `&["SEK", "USD", "CNY"]`).
/// * `months`            â€“ months in `"YYYY-MM"` format that must be available.
///
/// Returns the full (including previously cached) list of [`FxRateEntry`] items.
pub async fn sync_fx_rates(
    api_key: &str,
    database_path: &Path,
    base_currency: &str,
    currencies: &[&str],
    months: &[String],
) -> Result<Vec<FxRateEntry>> {
    let mut required_pairs = Vec::new();
    for month in months {
        for currency in currencies {
            if *currency != base_currency {
                required_pairs.push((month.clone(), (*currency).to_string()));
            }
        }
    }

    sync_fx_rates_for_pairs(api_key, database_path, base_currency, &required_pairs).await
}

/// Fetches and caches monthly average FX rates for the exact `(month,
/// from_currency)` pairs required by the data.
pub async fn sync_fx_rates_for_pairs(
    api_key: &str,
    database_path: &Path,
    base_currency: &str,
    required_pairs: &[(String, String)],
) -> Result<Vec<FxRateEntry>> {
    let mut cached = load_fx_rates(database_path)?;

    // Build a set of (month, from_currency) pairs already cached.
    let mut cached_pairs: HashSet<(String, String)> = cached
        .iter()
        .filter(|e| e.to_currency == base_currency)
        .map(|e| (e.month.clone(), e.from_currency.clone()))
        .collect();

    let mut missing_by_month: HashMap<String, Vec<String>> = HashMap::new();
    for (month, currency) in required_pairs {
        if currency == base_currency {
            continue;
        }

        let pair = (month.clone(), currency.clone());
        if cached_pairs.contains(&pair) {
            continue;
        }

        let month_currencies = missing_by_month.entry(month.clone()).or_default();
        if !month_currencies.iter().any(|existing| existing == currency) {
            month_currencies.push(currency.clone());
        }
    }

    if missing_by_month.is_empty() {
        save_fx_rates(database_path, &cached)?;
        return Ok(cached);
    }

    let mut missing_months: Vec<String> = missing_by_month.keys().cloned().collect();
    missing_months.sort();

    let client = reqwest::Client::new();
    let total_missing = missing_months.len();
    let mut newly_added = 0usize;
    let mut rate_limited_month: Option<String> = None;

    'months: for month in &missing_months {
        let source_currencies = missing_by_month
            .get(month)
            .cloned()
            .unwrap_or_default();
        let currencies_param = source_currencies.join(",");
        let date_1st = format!("{}-01", month);
        let date_28th = format!("{}-28", month);

        let rates_1st =
            fetch_rates_for_date(&client, api_key, base_currency, &date_1st, &currencies_param)
                .await;
        let rates_28th =
            fetch_rates_for_date(&client, api_key, base_currency, &date_28th, &currencies_param)
                .await;

        // Detect HTTP 429 rate-limit: if both samples are exhausted, save
        // whatever we have already cached and stop — no point burning more quota.
        let is_rate_limited = |e: &anyhow::Error| e.to_string().contains("429");

        // Merge the two sample dates: average when both are available, fall back
        // to whichever single date succeeded, and stop immediately on 429.
        let combined_rates: HashMap<String, f64> = match (rates_1st, rates_28th) {
            (Ok(r1), Ok(r28)) => {
                let mut combined = HashMap::new();
                for currency in &source_currencies {
                    match (r1.get(currency.as_str()), r28.get(currency.as_str())) {
                        (Some(&v1), Some(&v28)) => {
                            combined.insert(currency.to_string(), (v1 + v28) / 2.0);
                        }
                        (Some(&v1), None) => {
                            combined.insert(currency.to_string(), v1);
                        }
                        (None, Some(&v28)) => {
                            combined.insert(currency.to_string(), v28);
                        }
                        _ => {}
                    }
                }
                combined
            }
            (Ok(r1), Err(e)) => {
                if is_rate_limited(&e) {
                    // 28th was rate-limited; use only the 1st result but stop after
                    // this month to avoid wasting more quota.
                    eprintln!(
                        "⚠  FX warning: 28th-of-month fetch rate-limited for {}. Using 1st only.",
                        month
                    );
                    rate_limited_month = Some(month.to_string());
                    r1 // still cache what we got, then break below
                } else {
                    eprintln!(
                        "⚠  FX warning: 28th-of-month fetch failed for {} ({}). Falling back to 1st only.",
                        month, e
                    );
                    r1
                }
            }
            (Err(e), Ok(r28)) => {
                if is_rate_limited(&e) {
                    eprintln!(
                        "⚠  FX warning: 1st-of-month fetch rate-limited for {}. Using 28th only.",
                        month
                    );
                    rate_limited_month = Some(month.to_string());
                    r28
                } else {
                    eprintln!(
                        "⚠  FX warning: 1st-of-month fetch failed for {} ({}). Falling back to 28th only.",
                        month, e
                    );
                    r28
                }
            }
            (Err(e1), Err(e2)) => {
                // Both failed — if it's a rate-limit, save progress and stop.
                if is_rate_limited(&e1) || is_rate_limited(&e2) {
                    rate_limited_month = Some(month.to_string());
                    if newly_added > 0 {
                        save_fx_rates(database_path, &cached)?;
                    }
                    break 'months;
                }
                return Err(anyhow!(
                    "Failed to fetch FX rates for {} — 1st: {}; 28th: {}",
                    month,
                    e1,
                    e2
                ));
            }
        };

        for currency in &source_currencies {
            let pair = (month.to_string(), currency.to_string());
            if cached_pairs.contains(&pair) {
                continue;
            }
            if let Some(&rate) = combined_rates.get(currency.as_str()) {
                cached.push(FxRateEntry {
                    month: month.to_string(),
                    from_currency: currency.to_string(),
                    to_currency: base_currency.to_string(),
                    rate,
                });
                cached_pairs.insert(pair);
                newly_added += 1;
            }
        }

        // Save after every successful month so progress survives future rate-limits.
        if newly_added > 0 {
            save_fx_rates(database_path, &cached)?;
        }

        // Stop fetching after a rate-limited single-date month.
        if rate_limited_month.is_some() {
            break 'months;
        }
    }

    if let Some(ref blocked) = rate_limited_month {
        let fetched = missing_months
            .iter()
            .position(|m| m == blocked)
            .unwrap_or(0);
        let remaining = total_missing.saturating_sub(fetched);
        return Err(anyhow!(
            "API rate limit reached at {}. {}/{} missing months were cached this run; \
             {} month(s) still pending — re-run to continue filling the cache.",
            blocked,
            fetched,
            total_missing,
            remaining
        ));
    }

    Ok(cached)
}

// ---------------------------------------------------------------------------
// Utility: collect required months from a database JSON
// ---------------------------------------------------------------------------

/// Extracts all unique `"YYYY-MM"` months referenced by transactions,
/// positions, and balance references in a database [`Value`], along with the
/// exact non-base `(month, currency)` pairs they require for FX conversion.
pub fn collect_months_and_fx_pairs(
    db: &Value,
    base_currency: &str,
) -> (Vec<String>, Vec<(String, String)>) {
    let mut months: HashSet<String> = HashSet::new();
    let mut required_pairs: HashSet<(String, String)> = HashSet::new();

    if let Some(txns) = db.get("transactions").and_then(|v| v.as_array()) {
        for txn in txns {
            if let Some(date) = txn.get("date").and_then(|v| v.as_str()) {
                if date.len() >= 7 {
                    let month = date[..7].to_string();
                    months.insert(month.clone());
                    if let Some(cur) = txn.get("currency").and_then(|v| v.as_str()) {
                        if cur != base_currency {
                            required_pairs.insert((month, cur.to_string()));
                        }
                    }
                }
            }
        }
    }

    if let Some(positions) = db.get("positions").and_then(|v| v.as_array()) {
        for pos in positions {
            if let Some(date) = pos.get("as_of_date").and_then(|v| v.as_str()) {
                if date.len() >= 7 {
                    let month = date[..7].to_string();
                    months.insert(month.clone());
                    if let Some(cur) = pos.get("currency").and_then(|v| v.as_str()) {
                        if cur != base_currency {
                            required_pairs.insert((month, cur.to_string()));
                        }
                    }
                }
            }
        }
    }

    if let Some(references) = db.get("balance_references").and_then(|v| v.as_array()) {
        for reference in references {
            if let Some(date) = reference.get("date").and_then(|v| v.as_str()) {
                if date.len() >= 7 {
                    let month = date[..7].to_string();
                    months.insert(month.clone());
                    if let Some(cur) = reference.get("currency").and_then(|v| v.as_str()) {
                        if cur != base_currency {
                            required_pairs.insert((month, cur.to_string()));
                        }
                    }
                }
            }
        }
    }

    let mut months_vec: Vec<String> = months.into_iter().collect();
    months_vec.sort();
    let mut required_pairs_vec: Vec<(String, String)> = required_pairs.into_iter().collect();
    required_pairs_vec.sort();

    (months_vec, required_pairs_vec)
}

/// Extracts all unique `"YYYY-MM"` months referenced by transactions and
/// positions in a database [`Value`], along with every distinct non-base
/// currency code found in those records.
pub fn collect_months_and_currencies(
    db: &Value,
    base_currency: &str,
) -> (Vec<String>, Vec<String>) {
    let (months_vec, required_pairs) = collect_months_and_fx_pairs(db, base_currency);
    let currencies: HashSet<String> = required_pairs
        .into_iter()
        .map(|(_, currency)| currency)
        .collect();
    let mut currencies_vec: Vec<String> = currencies.into_iter().collect();
    currencies_vec.sort();

    (months_vec, currencies_vec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn collect_months_and_fx_pairs_keeps_exact_required_pairs() {
        let db = json!({
            "transactions": [
                {
                    "date": "2024-08-09",
                    "currency": "SEK"
                }
            ],
            "positions": [],
            "balance_references": [
                {
                    "date": "2025-10-10",
                    "currency": "CNY"
                },
                {
                    "date": "2025-11-01",
                    "currency": "EUR"
                }
            ]
        });

        let (months, required_pairs) = collect_months_and_fx_pairs(&db, "EUR");

        assert_eq!(
            months,
            vec![
                "2024-08".to_string(),
                "2025-10".to_string(),
                "2025-11".to_string()
            ]
        );
        assert_eq!(
            required_pairs,
            vec![
                ("2024-08".to_string(), "SEK".to_string()),
                ("2025-10".to_string(), "CNY".to_string()),
            ]
        );
    }

    #[test]
    fn collect_months_and_currencies_includes_balance_references() {
        let db = json!({
            "transactions": [],
            "positions": [],
            "balance_references": [
                {
                    "date": "2025-10-10",
                    "currency": "CNY"
                },
                {
                    "date": "2025-11-01",
                    "currency": "EUR"
                }
            ]
        });

        let (months, currencies) = collect_months_and_currencies(&db, "EUR");

        assert_eq!(months, vec!["2025-10".to_string(), "2025-11".to_string()]);
        assert_eq!(currencies, vec!["CNY".to_string()]);
    }
}
