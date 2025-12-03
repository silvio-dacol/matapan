use anyhow::{Context, Result, anyhow};
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashSet, fs, path::Path};

#[derive(Debug, Deserialize)]
struct SettingsHicp {
    base_year: i32,
    base_month: u32,
    base_value: f64,
}

#[derive(Debug, Deserialize)]
struct AccountCategories {
    assets: Vec<String>,
    liabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SpecialAccounts {
    external: String,
}

#[derive(Debug, Deserialize)]
struct SettingsFile {
    settings_version: u32,
    base_currency: String,
    hicp: SettingsHicp,
    account_categories: AccountCategories,
    transaction_user_categories: Vec<String>,
    special_accounts: SpecialAccounts,
}

#[derive(Default)]
struct Report {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl Report {
    fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(msg.into());
    }
    fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }
    fn print(&self, file: &str) {
        for w in &self.warnings {
            println!("[WARN] {}: {}", file, w);
        }
        for e in &self.errors {
            println!("[ERROR] {}: {}", file, e);
        }
    }
    fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

fn load_settings(path: &Path) -> Result<SettingsFile> {
    let txt = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let settings: SettingsFile = serde_json::from_str(&txt).context("parsing settings.json")?;
    Ok(settings)
}

fn validate_month_file(val: &Value, settings: &SettingsFile) -> Report {
    let mut rep = Report::default();

    // month format
    let month = val.get("month").and_then(|v| v.as_str()).unwrap_or("");
    let re = Regex::new(r"^\d{4}-\d{2}$").unwrap();
    if !re.is_match(month) {
        rep.error(format!(
            "invalid month format '{}', expected YYYY-MM",
            month
        ));
    }

    // fx_rates present and contains base currency
    let fx = val.get("fx_rates").and_then(|v| v.as_object());
    match fx {
        None => rep.error("missing 'fx_rates' object"),
        Some(map) => {
            if !map.contains_key(&settings.base_currency) {
                rep.error(format!(
                    "fx_rates missing base currency '{}'",
                    settings.base_currency
                ));
            } else if let Some(v) = map.get(&settings.base_currency).and_then(|v| v.as_f64()) {
                if (v - 1.0).abs() > 1e-6 {
                    rep.warn(format!(
                        "base currency '{}' rate is {}, expected 1.0",
                        settings.base_currency, v
                    ));
                }
            }
        }
    }

    // valid account types set
    let mut valid_account_types: HashSet<String> = settings
        .account_categories
        .assets
        .iter()
        .chain(settings.account_categories.liabilities.iter())
        .cloned()
        .collect();

    // validate net_worth_entries
    if let Some(entries) = val.get("net_worth_entries").and_then(|v| v.as_array()) {
        for (i, e) in entries.iter().enumerate() {
            let t = e.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let c = e.get("currency").and_then(|v| v.as_str()).unwrap_or("");
            let b = e.get("balance").and_then(|v| v.as_f64());
            if !valid_account_types.contains(t) {
                rep.warn(format!(
                    "net_worth_entries[{}] has unknown type '{}' (not listed in settings account_categories)",
                    i, t
                ));
            }
            match b {
                None => rep.error(format!("net_worth_entries[{}] missing 'balance'", i)),
                Some(_) => {}
            }
            if let Some(fx) = fx {
                if !fx.contains_key(c) {
                    rep.error(format!(
                        "net_worth_entries[{}] currency '{}' not found in fx_rates",
                        i, c
                    ));
                }
            }
        }
    } else {
        rep.warn("missing or non-array 'net_worth_entries'");
    }

    // validate cash-flow-entries
    if let Some(entries) = val
        .get("cash-flow-entries")
        .or_else(|| val.get("cash_flow_entries"))
        .and_then(|v| v.as_array())
    {
        let valid_tx: HashSet<&str> = settings
            .transaction_user_categories
            .iter()
            .map(|s| s.as_str())
            .collect();
        for (i, e) in entries.iter().enumerate() {
            let t = e.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let c = e.get("currency").and_then(|v| v.as_str()).unwrap_or("");
            let a = e.get("amount").and_then(|v| v.as_f64());
            if !valid_tx.contains(t) {
                rep.warn(format!(
                    "cash-flow-entries[{}] has user type '{}' not in settings.transaction_user_categories",
                    i, t
                ));
            }
            if a.is_none() {
                rep.error(format!("cash-flow-entries[{}] missing 'amount'", i));
            }
            if let Some(fx) = fx {
                if !fx.contains_key(c) {
                    rep.error(format!(
                        "cash-flow-entries[{}] currency '{}' not found in fx_rates",
                        i, c
                    ));
                }
            }
        }
    }

    // hicp numeric
    if val.get("hicp").and_then(|v| v.as_f64()).is_none() {
        rep.warn("missing or non-numeric 'hicp'");
    }

    rep
}

fn main() -> Result<()> {
    let root = Path::new(".");
    let settings = load_settings(&root.join("settings.json"))?;

    let db_dir = root.join("database");
    if !db_dir.exists() {
        return Err(anyhow!(
            "database directory not found: {}",
            db_dir.display()
        ));
    }

    let mut any_errors = false;
    for entry in fs::read_dir(&db_dir).context("listing database directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let txt =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let val: Value =
            serde_json::from_str(&txt).with_context(|| format!("parsing {}", path.display()))?;
        let report = validate_month_file(&val, &settings);
        report.print(file_name);
        if report.has_errors() {
            any_errors = true;
        }
    }

    if any_errors {
        Err(anyhow!("Validation failed"))
    } else {
        println!("All monthly files passed basic validation.");
        Ok(())
    }
}
