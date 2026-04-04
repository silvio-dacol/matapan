//! HICP (Harmonised Index of Consumer Prices) data loading, syncing, and lookup.
//!
//! HICP values are used to adjust transaction and position amounts for
//! inflation when building the normalised database.
//!
//! Data is fetched from the Eurostat dissemination API:
//! `https://ec.europa.eu/eurostat/api/dissemination/statistics/1.0/data/prc_hicp_midx`
//!
//! Dataset: `prc_hicp_midx` (monthly index, 2015 = 100), all-items HICP (`CP00`),
//! unit `I15`.  Country codes follow ISO 3166-1 alpha-2 (e.g. `"IT"`, `"SE"`).

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

/// A single HICP index entry for a given month and country.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HicpEntry {
    /// Month in `"YYYY-MM"` format.
    pub month: String,
    /// ISO 3166-1 alpha-2 country code (e.g., `"SE"`, `"IT"`).
    pub country: String,
    /// HICP index value for the month.
    pub value: f64,
}

// ---------------------------------------------------------------------------
// File I/O
// ---------------------------------------------------------------------------

fn hicp_path(database_path: &Path) -> PathBuf {
    let dir = if database_path.is_dir() {
        database_path.to_path_buf()
    } else {
        database_path
            .parent()
            .unwrap_or(database_path)
            .to_path_buf()
    };
    dir.join("hicp_series.json")
}

/// Loads HICP entries from `<database_dir>/hicp_series.json`.
///
/// Returns an empty vector if the file does not yet exist.
pub fn load_hicp(database_path: &Path) -> Result<Vec<HicpEntry>> {
    let path = hicp_path(database_path);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Cannot read hicp_series.json at {:?}", path))?;
    let entries: Vec<HicpEntry> = serde_json::from_str(&contents)
        .with_context(|| format!("Cannot parse hicp_series.json at {:?}", path))?;
    Ok(entries)
}

/// Saves HICP entries to `<database_dir>/hicp_series.json`.
///
/// Entries are sorted by `(country, month)` before writing.
pub fn save_hicp(database_path: &Path, entries: &[HicpEntry]) -> Result<()> {
    let path = hicp_path(database_path);
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| a.country.cmp(&b.country).then(a.month.cmp(&b.month)));
    let json = serde_json::to_string_pretty(&sorted)?;
    fs::write(&path, json)
        .with_context(|| format!("Cannot write hicp_series.json at {:?}", path))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Lookup
// ---------------------------------------------------------------------------

/// Returns the HICP index value for a given month and country.
///
/// If no entry is found, returns `1.0` (identity — no inflation adjustment).
pub fn lookup_hicp(entries: &[HicpEntry], month: &str, country: &str) -> f64 {
    entries
        .iter()
        .find(|e| e.month == month && e.country == country)
        .map(|e| e.value)
        .unwrap_or(1.0)
}

// ---------------------------------------------------------------------------
// API fetching (Eurostat dissemination API)
// ---------------------------------------------------------------------------

const EUROSTAT_HICP_URL: &str =
    "https://ec.europa.eu/eurostat/api/dissemination/statistics/1.0/data/prc_hicp_midx";

/// Fetches all available monthly HICP index values for `country` from the
/// Eurostat dissemination API (`prc_hicp_midx`, unit `I15`, coicop `CP00`).
///
/// Returns a map of `"YYYY-MM"` → HICP index value.
async fn fetch_hicp_country(
    client: &reqwest::Client,
    country: &str,
) -> Result<HashMap<String, f64>> {
    let response = client
        .get(EUROSTAT_HICP_URL)
        .query(&[
            ("format", "JSON"),
            ("geo", country),
            ("unit", "I15"),
            ("coicop", "CP00"),
        ])
        .send()
        .await
        .with_context(|| format!("Eurostat HICP request failed for country {}", country))?;

    let status = response.status();
    let body: Value = response
        .json()
        .await
        .with_context(|| format!("Failed to parse Eurostat HICP JSON for country {}", country))?;

    if !status.is_success() {
        return Err(anyhow!(
            "Eurostat API returned HTTP {} for country {}",
            status,
            country
        ));
    }

    // The SDMX-JSON 2.0 structure returned by Eurostat:
    //   dimension.time.category.index  →  { "YYYY-MM": <integer_idx>, … }
    //   value                          →  { "<integer_idx>": <f64>, … }
    let time_index = body
        .pointer("/dimension/time/category/index")
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            anyhow!(
                "Unexpected Eurostat response: missing dimension.time.category.index for country {}",
                country
            )
        })?;

    let values = body
        .get("value")
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            anyhow!(
                "Unexpected Eurostat response: missing 'value' object for country {}",
                country
            )
        })?;

    let mut result = HashMap::new();
    for (month, idx_val) in time_index {
        if let Some(idx) = idx_val.as_u64() {
            let key = idx.to_string();
            if let Some(val) = values.get(&key).and_then(|v| v.as_f64()) {
                result.insert(month.clone(), val);
            }
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Sync
// ---------------------------------------------------------------------------

/// Fetches and caches HICP entries for the given months and countries.
///
/// For each country that has at least one missing month in
/// `<database_dir>/hicp_series.json` the function calls the Eurostat API once
/// (retrieving the full available time series for that country), then stores
/// every newly resolved `(month, country)` pair into the cache.
///
/// # Arguments
/// * `database_path` – path to the database folder (or `database.json`; the
///                     parent directory is used as the cache location).
/// * `countries`     – ISO 3166-1 alpha-2 country codes to ensure are cached
///                     (e.g., `&["IT", "SE"]`).
/// * `months`        – months in `"YYYY-MM"` format that must be available.
///
/// Returns the full (including previously cached) list of [`HicpEntry`] items.
pub async fn sync_hicp(
    database_path: &Path,
    countries: &[&str],
    months: &[String],
) -> Result<Vec<HicpEntry>> {
    let mut cached = load_hicp(database_path)?;

    // Build a set of (month, country) pairs already in the cache.
    let cached_pairs: HashSet<(String, String)> = cached
        .iter()
        .map(|e| (e.month.clone(), e.country.clone()))
        .collect();

    // Find countries where at least one required month is missing.
    let countries_needing_fetch: Vec<&str> = countries
        .iter()
        .copied()
        .filter(|&c| {
            months
                .iter()
                .any(|m| !cached_pairs.contains(&(m.clone(), c.to_string())))
        })
        .collect();

    if countries_needing_fetch.is_empty() {
        save_hicp(database_path, &cached)?;
        return Ok(cached);
    }

    let client = reqwest::Client::new();
    let mut newly_added = 0usize;

    for &country in &countries_needing_fetch {
        let data = fetch_hicp_country(&client, country)
            .await
            .with_context(|| format!("Failed to sync HICP for country {}", country))?;

        for month in months {
            if cached_pairs.contains(&(month.clone(), country.to_string())) {
                continue;
            }
            if let Some(&value) = data.get(month.as_str()) {
                cached.push(HicpEntry {
                    month: month.clone(),
                    country: country.to_string(),
                    value,
                });
                newly_added += 1;
            }
            // Months not present in the API response (e.g. future months) are
            // silently skipped; lookup_hicp will fall back to 1.0.
        }
    }

    save_hicp(database_path, &cached)?;

    Ok(cached)
}
