use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use chrono::{NaiveDate, Utc};
use serde_json::Value;

use data_normalization::compute_adjustments;
use models::*;

pub struct Config {
    pub input_dir: PathBuf,
    pub output_file: PathBuf,
    pub latest_only: bool,
    pub pretty: bool,
}

/// Main pipeline function that processes net worth documents and generates dashboard output
pub fn run(cfg: Config) -> Result<()> {
    // Load all JSON documents from the input directory
    let mut docs = load_documents(&cfg.input_dir)?;

    // Sort by date ascending to ensure chronological order
    docs.sort_by_key(|d| {
        parse_date(&d.metadata.date)
            .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
    });

    // Handle the case where only the latest snapshot is requested
    if cfg.latest_only {
        if let Some(last) = docs.pop() {
            let snap = to_snapshot(&last)?;
            let dashboard = Dashboard {
                generated_at: Utc::now().to_rfc3339(),
                base_currency: last.metadata.base_currency.clone(),
                snapshots: vec![snap.clone()],
                latest: Some(snap),
            };
            write_dashboard(&cfg.output_file, &dashboard, cfg.pretty)?;
            return Ok(());
        } else {
            return Err(anyhow!("No input documents found"));
        }
    }

    // Process all documents into snapshots
    let mut snapshots = Vec::new();
    for doc in docs.iter() {
        let snap = to_snapshot(doc)?;
        snapshots.push(snap);
    }

    // Determine the base currency from the latest snapshot, defaulting to EUR
    let latest = snapshots.last().cloned();
    let base_currency = latest
        .as_ref()
        .map(|s| s.base_currency.clone())
        .unwrap_or_else(|| "EUR".to_string());

    // Create the final dashboard and write to output
    let dashboard = Dashboard {
        generated_at: Utc::now().to_rfc3339(),
        base_currency,
        snapshots,
        latest,
    };
    write_dashboard(&cfg.output_file, &dashboard, cfg.pretty)?;
    Ok(())
}

/// Writes the dashboard data to a JSON file with optional pretty formatting
fn write_dashboard(path: &PathBuf, dashboard: &Dashboard, pretty: bool) -> Result<()> {
    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Creating output dir: {}", parent.display()))?;
    }

    // Serialize dashboard to JSON (pretty or compact format)
    let json = if pretty {
        serde_json::to_string_pretty(dashboard)?
    } else {
        serde_json::to_string(dashboard)?
    };

    // Write JSON to file
    fs::write(path, json).with_context(|| format!("Writing output file: {}", path.display()))?;
    Ok(())
}

/// Loads and parses all JSON documents from the specified directory
fn load_documents(dir: &PathBuf) -> Result<Vec<InputDocument>> {
    let mut docs = Vec::new();

    // Read all entries in the directory
    let entries =
        fs::read_dir(dir).with_context(|| format!("Reading input dir: {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Skip directories
        if path.is_dir() {
            continue;
        }

        // Only process JSON files
        if path.extension().and_then(|s| s.to_str()).unwrap_or("") != "json" {
            continue;
        }

        // Skip template.json files
        if path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("template.json"))
            .unwrap_or(false)
        {
            continue;
        }

        // Read and parse the JSON file
        let raw =
            fs::read_to_string(&path).with_context(|| format!("Reading {}", path.display()))?;
        let json_val: Value = serde_json::from_str(&raw)
            .with_context(|| format!("Parsing JSON in {}", path.display()))?;
        let doc: InputDocument = serde_json::from_value(json_val)?;
        docs.push(doc);
    }
    Ok(docs)
}

/// Parses date strings in multiple formats (YYYY-MM-DD or YYYY/MM/DD)
fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(s, "%Y/%m/%d"))
        .map_err(|e| anyhow!(e))
}

/// Converts an input document into a snapshot with calculated totals and adjustments
fn to_snapshot(doc: &InputDocument) -> Result<Snapshot> {
    let date = parse_date(&doc.metadata.date)?;
    let base_currency = doc.metadata.base_currency.clone();

    let mut breakdown = SnapshotBreakdown::default();
    let mut warnings: Vec<String> = Vec::new();

    // Process each net worth entry and categorize by type
    for e in &doc.net_worth_entries {
        // Parse the category type
        let Some(cat) = Category::from_str(&e.kind) else {
            warnings.push(format!(
                "Unknown type '{}' for entry '{}' — skipped",
                e.kind, e.name
            ));
            continue;
        };

        // Convert currency to base currency using exchange rates
        let rate = fx_to_base(&e.currency, &base_currency, &doc.fx_rates);
        if rate.is_none() && e.currency.to_uppercase() != base_currency.to_uppercase() {
            warnings.push(format!(
                "Missing FX rate {}->{} for entry '{}' — assuming 1.0",
                e.currency, base_currency, e.name
            ));
        }
        let fx = rate.unwrap_or(1.0);
        let amount_base = e.balance * fx;

        // Add to the appropriate category breakdown
        match cat {
            Category::Cash => breakdown.cash += amount_base,
            Category::Investments => breakdown.investments += amount_base,
            Category::Personal => breakdown.personal += amount_base,
            Category::Pension => breakdown.pension += amount_base,
            Category::Liabilities => breakdown.liabilities += amount_base,
        }
    }

    // Calculate totals from breakdown
    let assets = breakdown.cash + breakdown.investments + breakdown.personal + breakdown.pension;
    let liabilities = breakdown.liabilities;
    let totals = SnapshotTotals {
        assets,
        liabilities,
        net_worth: assets - liabilities,
    };

    // Calculate various adjustments (inflation, cost-of-living, etc.)
    let (inflation_adjusted, new_york_normalized, real_purchasing_power) =
        compute_adjustments(doc, &breakdown, &totals, &mut warnings)?;

    Ok(Snapshot {
        date,
        base_currency,
        breakdown,
        totals,
        inflation_adjusted,
        new_york_normalized,
        real_purchasing_power,
        warnings,
    })
}

/// Converts currency amounts to base currency using exchange rates
fn fx_to_base(
    currency: &str,
    base: &str,
    rates: &std::collections::HashMap<String, f64>,
) -> Option<f64> {
    // Same currency - no conversion needed
    if currency.eq_ignore_ascii_case(base) {
        return Some(1.0);
    }

    // Look up exchange rate (1 unit of 'currency' equals 'rate' units of base currency)
    // Accept both upper and lower case keys for flexibility
    rates
        .get(&currency.to_uppercase())
        .copied()
        .or_else(|| rates.get(&currency.to_string()).copied())
}
