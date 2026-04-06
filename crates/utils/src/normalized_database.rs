//! Builds a normalised "child" database from a master `database.json`.
//!
//! The normalised database is a copy of the master database where every
//! transaction and position has two additional fields injected and all
//! monetary amounts are converted to the user's `base_currency`:
//!
//! * `exchange_rate` – units of the original currency per one unit of
//!   `base_currency` (e.g., `10.4` for SEK→EUR means 10.4 SEK = 1 EUR).
//!   Set to `1.0` when the record is already in the base currency.
//! * `hicp` – HICP inflation index for the record's month and the user's
//!   `tax_residency` country.  Defaults to `1.0` while the inflation feed
//!   is not yet connected.
//!
//! The normalised file is written to `<database_dir>/database_normalized.json`
//! and is kept in sync with the master database by calling
//! [`sync_normalized_database`].

use anyhow::{anyhow, Context, Result};
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use crate::{
    balance_references::compute_monthly_balances,
    fx_rates::{collect_months_and_currencies, lookup_rate, sync_fx_rates, FxRateEntry},
    hicp::{load_hicp, lookup_hicp, sync_hicp, HicpEntry},
    round_digits::round_money,
};

// ---------------------------------------------------------------------------
// File paths
// ---------------------------------------------------------------------------

fn normalized_db_path(database_path: &Path) -> PathBuf {
    let dir = if database_path.is_dir() {
        database_path.to_path_buf()
    } else {
        database_path
            .parent()
            .unwrap_or(database_path)
            .to_path_buf()
    };
    dir.join("database_normalized.json")
}

// ---------------------------------------------------------------------------
// Core transformation helpers
// ---------------------------------------------------------------------------

/// Converts a monetary value to the base currency using the exchange rate.
///
/// `rate` = units of original currency per 1 base currency unit.
/// Conversion: `base_amount = original_amount / rate`.
fn to_base(amount: f64, rate: f64) -> f64 {
    if rate == 0.0 {
        return 0.0;
    }
    round_money(amount / rate)
}

/// Normalises a single transaction object in-place.
///
/// Adds `exchange_rate` and `hicp` fields, converts `amount` to the base
/// currency, and updates `currency` to `base_currency`.
/// Returns an error if the required FX rate is missing from the cache.
fn normalise_transaction(
    txn: &mut Map<String, Value>,
    fx_rates: &[FxRateEntry],
    hicp_entries: &[HicpEntry],
    base_currency: &str,
    tax_residency: &str,
) -> Result<()> {
    let currency = txn
        .get("currency")
        .and_then(|v| v.as_str())
        .unwrap_or(base_currency)
        .to_string();

    let date = txn
        .get("date")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let month = if date.len() >= 7 { &date[..7] } else { "" };

    // Exchange rate (units of original currency per 1 base_currency).
    let rate = lookup_rate(fx_rates, month, &currency, base_currency).ok_or_else(|| {
        let txn_id = txn
            .get("txn_id")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        anyhow!(
            "Missing FX rate for {}/{} in month {} (transaction {}). \
             Run sync_fx_rates first.",
            currency,
            base_currency,
            month,
            txn_id
        )
    })?;

    let hicp = lookup_hicp(hicp_entries, month, tax_residency);

    // Convert amount.
    if let Some(amount_val) = txn.get("amount").and_then(|v| v.as_f64()) {
        let converted = to_base(amount_val, rate);
        txn.insert("amount".to_string(), Value::from(converted));
    }

    // Inject extra fields and update currency.
    txn.insert("exchange_rate".to_string(), Value::from(rate));
    txn.insert("hicp".to_string(), Value::from(hicp));
    txn.insert("currency".to_string(), Value::String(base_currency.to_string()));

    Ok(())
}

/// Normalises a single position object in-place.
///
/// Adds `exchange_rate` and `hicp` fields, converts all monetary fields to
/// the base currency, and updates `currency` to `base_currency`.
/// Returns an error if the required FX rate is missing from the cache.
fn normalise_position(
    pos: &mut Map<String, Value>,
    fx_rates: &[FxRateEntry],
    hicp_entries: &[HicpEntry],
    base_currency: &str,
    tax_residency: &str,
) -> Result<()> {
    let currency = pos
        .get("currency")
        .and_then(|v| v.as_str())
        .unwrap_or(base_currency)
        .to_string();

    let date = pos
        .get("as_of_date")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let month = if date.len() >= 7 { &date[..7] } else { "" };

    let rate = lookup_rate(fx_rates, month, &currency, base_currency).ok_or_else(|| {
        let pos_id = pos
            .get("position_id")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        anyhow!(
            "Missing FX rate for {}/{} in month {} (position {}). \
             Run sync_fx_rates first.",
            currency,
            base_currency,
            month,
            pos_id
        )
    })?;

    let hicp = lookup_hicp(hicp_entries, month, tax_residency);

    // Convert monetary fields.
    for field in &[
        "cost_price",
        "cost_basis",
        "close_price",
        "market_value",
        "unrealized_profit",
        "unrealized_loss",
    ] {
        if let Some(val) = pos.get(*field).and_then(|v| v.as_f64()) {
            pos.insert(field.to_string(), Value::from(to_base(val, rate)));
        }
    }

    pos.insert("exchange_rate".to_string(), Value::from(rate));
    pos.insert("hicp".to_string(), Value::from(hicp));
    pos.insert("currency".to_string(), Value::String(base_currency.to_string()));

    Ok(())
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Builds a normalised database [`Value`] from a master database.
///
/// The FX rate cache (`fx_rates`) and the HICP series (`hicp_entries`) must
/// already be loaded; use [`sync_normalized_database`] for the all-in-one
/// pipeline that also handles fetching.
///
/// The function returns an error if any transaction or position references a
/// currency/month pair that is absent from `fx_rates`.
pub fn build_normalized_database(
    source_db: &Value,
    fx_rates: &[FxRateEntry],
    hicp_entries: &[HicpEntry],
) -> Result<Value> {
    let mut normalised = source_db.clone();

    let base_currency = normalised
        .get("user_profile")
        .and_then(|p| p.get("base_currency"))
        .and_then(|v| v.as_str())
        .unwrap_or("EUR")
        .to_string();

    let tax_residency = normalised
        .get("user_profile")
        .and_then(|p| p.get("tax_residency"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Normalise transactions.
    if let Some(txns) = normalised
        .get_mut("transactions")
        .and_then(|v| v.as_array_mut())
    {
        for txn in txns.iter_mut() {
            if let Some(obj) = txn.as_object_mut() {
                normalise_transaction(
                    obj,
                    fx_rates,
                    hicp_entries,
                    &base_currency,
                    &tax_residency,
                )?;
            }
        }
    }

    // Normalise positions.
    if let Some(positions) = normalised
        .get_mut("positions")
        .and_then(|v| v.as_array_mut())
    {
        for pos in positions.iter_mut() {
            if let Some(obj) = pos.as_object_mut() {
                normalise_position(
                    obj,
                    fx_rates,
                    hicp_entries,
                    &base_currency,
                    &tax_residency,
                )?;
            }
        }
    }

    // Build month_end_snapshots from balance_references + normalised transactions.
    let snapshots = build_month_end_snapshots(&normalised, &base_currency);
    normalised["month_end_snapshots"] = serde_json::Value::Array(snapshots);

    Ok(normalised)
}

/// Derives monthly end-of-balance snapshots from every entry in
/// `balance_references`, using the already-normalised (base-currency)
/// transactions that are present in `normalised_db`.
///
/// Each snapshot has the shape:
/// ```json
/// {
///   "account_id": "SEB_SAVINGS",
///   "month": "2024-03",
///   "balance": 42500.00,
///   "currency": "EUR",
///   "source_reference_id": "SEB_SAVINGS-2024-03-31"
/// }
/// ```
///
/// When multiple references exist for the same account, the one whose date is
/// closest to each target month is used (minimises accumulated rounding drift).
/// Duplicate `(account_id, month)` pairs are merged by keeping the entry from
/// the nearest reference.
fn build_month_end_snapshots(normalised_db: &Value, base_currency: &str) -> Vec<Value> {
    let refs = match normalised_db
        .get("balance_references")
        .and_then(|v| v.as_array())
    {
        Some(arr) => arr,
        None => return vec![],
    };

    if refs.is_empty() {
        return vec![];
    }

    let txns: &[Value] = normalised_db
        .get("transactions")
        .and_then(|v| v.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    // Keyed by (account_id, month) → (balance, reference_id, ref_month).
    // We keep only the entry produced by the reference whose month is closest
    // to the target month, to minimise accumulated drift.
    let mut best: BTreeMap<(String, String), (f64, String, String)> = BTreeMap::new();

    for reference in refs {
        let ref_id = reference
            .get("reference_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let ref_date = match reference.get("date").and_then(|v| v.as_str()) {
            Some(d) if d.len() >= 7 => d,
            _ => continue,
        };
        let ref_month = &ref_date[..7];

        let monthly = match compute_monthly_balances(reference, txns) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let account_id = match reference.get("account_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => continue,
        };

        for (month, balance) in &monthly {
            let key = (account_id.to_string(), month.clone());
            let distance = month_distance(month, ref_month);

            let replace = match best.get(&key) {
                None => true,
                Some((_, _, existing_ref_month)) => {
                    month_distance(month, existing_ref_month) > distance
                }
            };

            if replace {
                best.insert(key, (*balance, ref_id.to_string(), ref_month.to_string()));
            }
        }
    }

    best.into_iter()
        .map(|((account_id, month), (balance, ref_id, _))| {
            serde_json::json!({
                "account_id": account_id,
                "month": month,
                "balance": balance,
                "currency": base_currency,
                "source_reference_id": ref_id,
            })
        })
        .collect()
}

/// Returns the absolute calendar-month distance between two `"YYYY-MM"` strings.
fn month_distance(a: &str, b: &str) -> u32 {
    fn to_months(s: &str) -> i32 {
        let year: i32 = s.get(..4).and_then(|y| y.parse().ok()).unwrap_or(0);
        let month: i32 = s.get(5..7).and_then(|m| m.parse().ok()).unwrap_or(0);
        year * 12 + month
    }
    to_months(a).abs_diff(to_months(b))
}

/// Full pipeline: ensures FX rates are up to date, then rebuilds and saves
/// `database_normalized.json` alongside `database.json`.
///
/// Steps performed:
/// 1. Read `database.json` from `database_path`.
/// 2. Collect the currencies and months referenced by transactions/positions.
/// 3. Fetch any missing monthly FX rates from freecurrencyapi.com and cache them.
/// 4. Load the HICP series (stub — returns 1.0 everywhere until integrated).
/// 5. Build the normalised database.
/// 6. Write `database_normalized.json` to the same directory.
///
/// # Arguments
/// * `database_path` – path to `database.json` or its parent directory.
/// * `api_key`        – freecurrencyapi.com API key.
pub async fn sync_normalized_database(database_path: &Path, api_key: &str) -> Result<()> {
    // Resolve the directory.
    let db_file = if database_path.is_dir() {
        database_path.join("database.json")
    } else {
        database_path.to_path_buf()
    };

    let db_dir = db_file.parent().unwrap_or(database_path);

    // Load master database.
    let contents = fs::read_to_string(&db_file)
        .with_context(|| format!("Cannot read source database at {:?}", db_file))?;
    let source_db: Value = serde_json::from_str(&contents)
        .with_context(|| format!("Cannot parse database JSON at {:?}", db_file))?;

    let base_currency = source_db
        .get("user_profile")
        .and_then(|p| p.get("base_currency"))
        .and_then(|v| v.as_str())
        .unwrap_or("EUR")
        .to_string();

    let tax_residency = source_db
        .get("user_profile")
        .and_then(|p| p.get("tax_residency"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Collect months and currencies from the source database.
    let (months, currencies) = collect_months_and_currencies(&source_db, &base_currency);

    // Run FX and HICP syncs concurrently — they use independent APIs so
    // neither blocks the other (e.g. an FX 429 won't prevent HICP from running).
    let currency_refs: Vec<&str> = currencies.iter().map(String::as_str).collect();
    let hicp_countries: Vec<&str> = if tax_residency.is_empty() {
        vec![]
    } else {
        vec![tax_residency.as_str()]
    };

    let (fx_result, hicp_result) = tokio::join!(
        sync_fx_rates(api_key, db_dir, &base_currency, &currency_refs, &months),
        async {
            if hicp_countries.is_empty() {
                load_hicp(db_dir)
            } else {
                sync_hicp(db_dir, &hicp_countries, &months).await
            }
        }
    );

    // Surface HICP errors immediately (Eurostat is reliable; errors are unexpected).
    let hicp_entries = hicp_result?;

    // FX errors (e.g. rate-limit) are non-fatal for the HICP cache but still
    // prevent building the normalised database — propagate after HICP is saved.
    let fx_rates = fx_result?;

    // Build normalised database and write it.
    let normalised = build_normalized_database(&source_db, &fx_rates, &hicp_entries)?;

    let out_path: PathBuf = normalized_db_path(database_path);
    let json = serde_json::to_string_pretty(&normalised)?;
    fs::write(&out_path, json)
        .with_context(|| format!("Cannot write normalised database at {:?}", out_path))?;

    Ok(())
}

/// Synchronous wrapper around [`sync_normalized_database`].
///
/// Spawns a single-threaded Tokio runtime for the duration of the call,
/// making it safe to invoke from any synchronous context (e.g. a parser
/// binary's `main` or the shared pipeline CLI entry point).
///
/// Returns immediately without error when `api_key` is empty.
pub fn sync_normalized_database_blocking(database_path: &Path, api_key: &str) -> Result<()> {
    if api_key.is_empty() {
        return Ok(());
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to build Tokio runtime for FX sync")?;
    rt.block_on(sync_normalized_database(database_path, api_key))
}
