//! HICP (Harmonised Index of Consumer Prices) data loading and lookup.
//!
//! HICP values are used to adjust transaction and position amounts for
//! inflation when building the normalised database.
//!
//! At present the API integration is not yet implemented.  All lookups
//! return `1.0`, leaving amounts unchanged.  The data model and file
//! I/O are in place for when the inflation feed is connected.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
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
pub fn save_hicp(database_path: &Path, entries: &[HicpEntry]) -> Result<()> {
    let path = hicp_path(database_path);
    let json = serde_json::to_string_pretty(entries)?;
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
/// This is the correct behaviour while the inflation feed is not yet connected.
pub fn lookup_hicp(entries: &[HicpEntry], month: &str, country: &str) -> f64 {
    entries
        .iter()
        .find(|e| e.month == month && e.country == country)
        .map(|e| e.value)
        .unwrap_or(1.0)
}
