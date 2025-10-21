use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use chrono::{NaiveDate, Utc};
use serde_json::Value;

use crate::models::*;

pub struct Config {
    pub input_dir: PathBuf,
    pub output_file: PathBuf,
    pub latest_only: bool,
    pub pretty: bool,
}

pub fn run(cfg: Config) -> Result<()> {
    let mut docs = load_documents(&cfg.input_dir)?;

    // Sort by date ascending
    docs.sort_by_key(|d| {
        parse_date(&d.metadata.date)
            .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
    });

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

    let mut snapshots = Vec::new();
    for doc in docs.iter() {
        let snap = to_snapshot(doc)?;
        snapshots.push(snap);
    }

    let latest = snapshots.last().cloned();
    let base_currency = latest
        .as_ref()
        .map(|s| s.base_currency.clone())
        .unwrap_or_else(|| "EUR".to_string());

    let dashboard = Dashboard {
        generated_at: Utc::now().to_rfc3339(),
        base_currency,
        snapshots,
        latest,
    };
    write_dashboard(&cfg.output_file, &dashboard, cfg.pretty)?;
    Ok(())
}

fn write_dashboard(path: &PathBuf, dashboard: &Dashboard, pretty: bool) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Creating output dir: {}", parent.display()))?;
    }
    let json = if pretty {
        serde_json::to_string_pretty(dashboard)?
    } else {
        serde_json::to_string(dashboard)?
    };
    fs::write(path, json).with_context(|| format!("Writing output file: {}", path.display()))?;
    Ok(())
}

fn load_documents(dir: &PathBuf) -> Result<Vec<InputDocument>> {
    let mut docs = Vec::new();
    let entries =
        fs::read_dir(dir).with_context(|| format!("Reading input dir: {}", dir.display()))?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()).unwrap_or("") != "json" {
            continue;
        }
        if path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("template.json"))
            .unwrap_or(false)
        {
            continue;
        }

        let raw =
            fs::read_to_string(&path).with_context(|| format!("Reading {}", path.display()))?;
        let json_val: Value = serde_json::from_str(&raw)
            .with_context(|| format!("Parsing JSON in {}", path.display()))?;
        let doc: InputDocument = serde_json::from_value(json_val)?;
        docs.push(doc);
    }
    Ok(docs)
}

fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(s, "%Y/%m/%d"))
        .map_err(|e| anyhow!(e))
}

fn to_snapshot(doc: &InputDocument) -> Result<Snapshot> {
    let date = parse_date(&doc.metadata.date)?;
    let base_currency = doc.metadata.base_currency.clone();

    let mut breakdown = SnapshotBreakdown::default();
    let mut warnings: Vec<String> = Vec::new();

    for e in &doc.net_worth_entries {
        let Some(cat) = Category::from_str(&e.kind) else {
            warnings.push(format!(
                "Unknown type '{}' for entry '{}' — skipped",
                e.kind, e.name
            ));
            continue;
        };
        let rate = fx_to_base(&e.currency, &base_currency, &doc.fx_rates);
        if rate.is_none() && e.currency.to_uppercase() != base_currency.to_uppercase() {
            warnings.push(format!(
                "Missing FX rate {}->{} for entry '{}' — assuming 1.0",
                e.currency, base_currency, e.name
            ));
        }
        let fx = rate.unwrap_or(1.0);
        let amount_base = e.balance * fx;

        match cat {
            Category::Cash => breakdown.cash += amount_base,
            Category::Investments => breakdown.investments += amount_base,
            Category::Personal => breakdown.personal += amount_base,
            Category::Pension => breakdown.pension += amount_base,
            Category::Liabilities => breakdown.liabilities += amount_base,
        }
    }

    let assets = breakdown.cash + breakdown.investments + breakdown.personal + breakdown.pension;
    let liabilities = breakdown.liabilities;
    let totals = SnapshotTotals {
        assets,
        liabilities,
        net_worth: assets - liabilities,
    };

    let normalized = compute_normalized(doc, &breakdown, &totals)?;

    Ok(Snapshot {
        date,
        base_currency,
        breakdown,
        totals,
        normalized,
        warnings,
    })
}

fn fx_to_base(
    currency: &str,
    base: &str,
    rates: &std::collections::HashMap<String, f64>,
) -> Option<f64> {
    if currency.eq_ignore_ascii_case(base) {
        return Some(1.0);
    }
    // Interpret as: 1 unit of 'currency' equals 'rate' units of base currency.
    // Accept both upper and lower case keys.
    rates
        .get(&currency.to_uppercase())
        .copied()
        .or_else(|| rates.get(&currency.to_string()).copied())
}

fn compute_normalized(
    doc: &InputDocument,
    b: &SnapshotBreakdown,
    _t: &SnapshotTotals,
) -> Result<Option<SnapshotNormalized>> {
    let normalize = doc
        .metadata
        .normalize
        .clone()
        .unwrap_or_else(|| "no".to_string());
    if normalize.to_lowercase() != "yes" {
        return Ok(None);
    }

    let Some(hicp) = &doc.metadata.hicp else {
        return Ok(None);
    };
    let Some(curr_hicp) = doc.inflation.current_hicp else {
        return Ok(None);
    };
    let Some(ecli_basic) = &doc.inflation.ecli_basic else {
        return Ok(None);
    };
    let Some(weights) = &doc.metadata.ecli_weight else {
        return Ok(None);
    };

    let deflator = hicp.base_hicp / curr_hicp;
    let ecli = weights.rent_index_weight * ecli_basic.rent_index
        + weights.groceries_index_weight * ecli_basic.groceries_index
        + weights.cost_of_living_index_weight * ecli_basic.cost_of_living_index;
    let ecli_norm = if ecli.abs() < f64::EPSILON {
        1.0
    } else {
        ecli / 100.0
    };

    let scale = deflator / ecli_norm;

    let nb = SnapshotBreakdown {
        cash: b.cash * scale,
        investments: b.investments * scale,
        personal: b.personal * scale,
        pension: b.pension * scale,
        liabilities: b.liabilities * scale,
    };
    let na = nb.cash + nb.investments + nb.personal + nb.pension;
    let nt = SnapshotTotals {
        assets: na,
        liabilities: nb.liabilities,
        net_worth: na - nb.liabilities,
    };

    Ok(Some(SnapshotNormalized {
        breakdown: nb,
        totals: nt,
        deflator,
        ecli_norm,
    }))
}
