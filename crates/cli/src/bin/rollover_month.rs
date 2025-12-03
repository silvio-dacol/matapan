use anyhow::{anyhow, Context, Result};
use chrono::{Datelike, NaiveDate};
use clap::Parser;
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

#[derive(Parser, Debug)]
#[command(name = "rollover-month", about = "Generate next month JSON by copying accounts and balances.")]
struct Args {
    /// Path to current month JSON file (e.g., database/2025_01.json)
    #[arg(short, long)]
    input: PathBuf,

    /// Optional output path; defaults to database/<next>_MM.json
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// If set, keep fx_rates and hicp from input (default true)
    #[arg(long, default_value_t = true)]
    keep_meta: bool,
}

fn next_month_string(month_str: &str) -> Result<String> {
    // month_str like "YYYY-MM"
    let parts: Vec<&str> = month_str.split('-').collect();
    if parts.len() != 2 { return Err(anyhow!("invalid month format: {}", month_str)); }
    let y: i32 = parts[0].parse().map_err(|_| anyhow!("invalid year in month"))?;
    let m: u32 = parts[1].parse().map_err(|_| anyhow!("invalid month in month"))?;
    let day = NaiveDate::from_ymd_opt(y, m, 1).ok_or_else(|| anyhow!("invalid date"))?;
    let next = if m == 12 { NaiveDate::from_ymd_opt(y + 1, 1, 1).unwrap() } else { NaiveDate::from_ymd_opt(y, m + 1, 1).unwrap() };
    Ok(format!("{}-{:02}", next.year(), next.month()))
}

fn default_output_path(input: &PathBuf, next_month: &str) -> PathBuf {
    // Use database/<YYYY_MM>.json (underscore as in repository)
    let db_dir = input.parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let fname = next_month.replace('-', "_") + ".json";
    db_dir.join(fname)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let txt = fs::read_to_string(&args.input).with_context(|| format!("reading {}", args.input.display()))?;
    let val: Value = serde_json::from_str(&txt).with_context(|| format!("parsing {}", args.input.display()))?;

    let month = val.get("month").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("input missing 'month'"))?;
    let next_month = next_month_string(month)?;

    // Copy net_worth_entries as-is; reset transactional arrays
    let net_worth_entries = val.get("net_worth_entries").cloned().unwrap_or(json!([]));
    let fx_rates = if args.keep_meta { val.get("fx_rates").cloned().unwrap_or(json!({})) } else { json!({}) };
    let hicp = if args.keep_meta { val.get("hicp").cloned().unwrap_or(json!(null)) } else { json!(null) };

    let out = json!({
        "month": next_month,
        "fx_rates": fx_rates,
        "hicp": hicp,
        "cash-flow-entries": [],
        "movements": [],
        "net_worth_entries": net_worth_entries,
        "investment_contributions": []
    });

    let out_path = args.output.unwrap_or_else(|| default_output_path(&args.input, &next_month));
    fs::write(&out_path, serde_json::to_string_pretty(&out)?).with_context(|| format!("writing {}", out_path.display()))?;
    println!("Generated next month file: {}", out_path.display());
    Ok(())
}
