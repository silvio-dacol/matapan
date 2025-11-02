use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use chrono::{NaiveDate, Utc};

use data_normalization::compute_adjustments;
use models::*;
use settings_loader;

pub struct Config {
    pub input_dir: PathBuf,
    pub output_file: PathBuf,
    pub settings_file: Option<PathBuf>,
    pub latest_only: bool,
    pub pretty: bool,
}

/// Main pipeline function that processes net worth documents and generates dashboard output
pub fn run(cfg: Config) -> Result<()> {
    // Load settings if provided
    let settings = settings_loader::load_optional_settings(cfg.settings_file.as_ref())?;

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
            let snap = to_snapshot(&last, settings.as_ref())?;
            let dashboard = Dashboard {
                generated_at: Utc::now().to_rfc3339(),
                base_currency: last.get_base_currency(settings.as_ref()),
                snapshots: vec![snap.clone()],
                latest: Some(snap),
            };

            // Round all values to 2 decimal places before writing
            let rounded_dashboard = dashboard.rounded();

            write_dashboard(&cfg.output_file, &rounded_dashboard, cfg.pretty)?;
            return Ok(());
        } else {
            return Err(anyhow!("No input documents found"));
        }
    }

    // Process all documents into snapshots
    let mut snapshots = Vec::new();
    for doc in docs.iter() {
        let snap = to_snapshot(doc, settings.as_ref())?;
        snapshots.push(snap);
    }

    // Determine the base currency from the latest snapshot, defaulting to settings or EUR
    let latest = snapshots.last().cloned();
    let base_currency = latest
        .as_ref()
        .map(|s| s.base_currency.clone())
        .or_else(|| settings.as_ref().map(|s| s.base_currency.clone()))
        .unwrap_or_else(|| "EUR".to_string());

    // Create the final dashboard and write to output
    let dashboard = Dashboard {
        generated_at: Utc::now().to_rfc3339(),
        base_currency,
        snapshots,
        latest,
    };

    // Round all values to 2 decimal places before writing
    let rounded_dashboard = dashboard.rounded();

    write_dashboard(&cfg.output_file, &rounded_dashboard, cfg.pretty)?;
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
        if !path.extension().is_some_and(|ext| ext == "json") {
            continue;
        }

        // Skip template.json and dashboard.json files, and hidden files
        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
            let name_lower = filename.to_ascii_lowercase();
            if name_lower == "template.json"
                || name_lower == "dashboard.json"
                || filename.starts_with('.')
            {
                continue;
            }
        }

        // Read and parse the JSON file directly without intermediate Value
        let raw =
            fs::read_to_string(&path).with_context(|| format!("Reading {}", path.display()))?;
        let doc: InputDocument = serde_json::from_str(&raw)
            .with_context(|| format!("Parsing JSON in {}", path.display()))?;
        docs.push(doc);
    }
    Ok(docs)
}

/// Parses date strings in multiple formats (YYYY-MM-DD or YYYY/MM/DD)
#[inline]
fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(s, "%Y/%m/%d"))
        .map_err(|e| anyhow!(e))
}

/// Converts an input document into a snapshot with calculated totals and adjustments
fn to_snapshot(doc: &InputDocument, settings: Option<&Settings>) -> Result<Snapshot> {
    let date = parse_date(&doc.metadata.date)?;
    let base_currency = doc.get_base_currency(settings);

    let mut breakdown = SnapshotBreakdown::default();
    let mut warnings = Vec::new();

    // Get fx_rates from document (handles both old and new formats)
    let fx_rates = doc.get_fx_rates();

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
        let rate = fx_to_base(&e.currency, &base_currency, &fx_rates);
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
    let (inflation_adjusted, real_purchasing_power) =
        compute_adjustments(doc, settings, &totals, &breakdown, &mut warnings)?;

    // Extract metadata for the period
    let fx_rates_opt = if fx_rates.is_empty() {
        None
    } else {
        Some(fx_rates)
    };
    let hicp_opt = doc.metadata.hicp;
    let ecli_opt = doc.metadata.ecli.clone();
    let reference_month = doc.metadata.reference_month.clone();

    Ok(Snapshot {
        date,
        reference_month,
        base_currency,
        fx_rates: fx_rates_opt,
        hicp: hicp_opt,
        ecli: ecli_opt,
        breakdown,
        totals,
        inflation_adjusted,
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
    // Try exact match first, then uppercase
    rates
        .get(currency)
        .or_else(|| rates.get(&currency.to_uppercase()))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_fx_to_base_same_currency() {
        let rates = HashMap::new();
        let result = fx_to_base("EUR", "EUR", &rates);
        assert_eq!(result, Some(1.0));

        // Case insensitive
        let result = fx_to_base("eur", "EUR", &rates);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn test_fx_to_base_conversion() {
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), 0.85);
        rates.insert("GBP".to_string(), 1.15);

        let result = fx_to_base("USD", "EUR", &rates);
        assert_eq!(result, Some(0.85));

        let result = fx_to_base("GBP", "EUR", &rates);
        assert_eq!(result, Some(1.15));
    }

    #[test]
    fn test_fx_to_base_missing_rate() {
        let rates = HashMap::new();
        let result = fx_to_base("JPY", "EUR", &rates);
        assert_eq!(result, None);
    }

    #[test]
    fn test_fx_to_base_case_insensitive_lookup() {
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), 0.85);

        let result = fx_to_base("usd", "EUR", &rates);
        assert_eq!(result, Some(0.85));
    }

    #[test]
    fn test_parse_date_dash_format() {
        let result = parse_date("2024-09-10");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_string(), "2024-09-10");
    }

    #[test]
    fn test_parse_date_slash_format() {
        let result = parse_date("2024/09/10");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_string(), "2024-09-10");
    }

    #[test]
    fn test_parse_date_invalid() {
        let result = parse_date("invalid-date");
        assert!(result.is_err());

        let result = parse_date("2024-13-01"); // Invalid month
        assert!(result.is_err());
    }

    #[test]
    fn test_category_from_str() {
        use models::Category;

        assert_eq!(Category::from_str("cash"), Some(Category::Cash));
        assert_eq!(Category::from_str("Cash"), Some(Category::Cash));
        assert_eq!(Category::from_str("liquidity"), Some(Category::Cash));

        assert_eq!(
            Category::from_str("investments"),
            Some(Category::Investments)
        );
        assert_eq!(Category::from_str("Investments"), Some(Category::Investments));

        assert_eq!(Category::from_str("pension"), Some(Category::Pension));
        assert_eq!(Category::from_str("retirement"), Some(Category::Pension));

        assert_eq!(
            Category::from_str("liabilities"),
            Some(Category::Liabilities)
        );
        assert_eq!(Category::from_str("debt"), Some(Category::Liabilities));

        assert_eq!(Category::from_str("unknown"), None);
    }
}
