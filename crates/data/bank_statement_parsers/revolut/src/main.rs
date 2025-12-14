use chrono::{NaiveDate, NaiveDateTime};
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::path::Path;
#[derive(Debug, Deserialize, Default)]
struct Settings {
    /// Map Revolut Product → account_id in the engine (e.g. {"Current": "SE_CHECKING"})
    #[serde(default)]
    account_map: HashMap<String, String>,
    /// Category rules: keyword (or regex) matched in description → category
    /// Rules are applied in insertion order; first match wins.
    #[serde(default)]
    category_rules: Vec<CategoryRule>,
    /// Optional mapping to override transaction type based on final category
    /// e.g. {"topup": "internal_transfer"}
    #[serde(default)]
    category_type_map: HashMap<String, String>,
    /// Contra-account rules: keyword/regex in description → contra_account_id
    #[serde(default)]
    contra_account_rules: Vec<ContraAccountRule>,
    /// Optional tags to add when a rule matches
    #[serde(default)]
    default_tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CategoryRule {
    /// Simple substring match (lowercased) or regex if `is_regex=true`
    pattern: String,
    #[serde(default)]
    is_regex: bool,
    category: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ContraAccountRule {
    /// Simple substring match (lowercased) or regex if `is_regex=true`
    pattern: String,
    #[serde(default)]
    is_regex: bool,
    contra_account_id: String,
}

fn load_settings<P: AsRef<Path>>(path: P) -> Settings {
    let content = std::fs::read_to_string(path);
    match content {
        Ok(c) => serde_json::from_str::<Settings>(&c).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

/// One row from the Revolut export
#[derive(Debug, Deserialize)]
struct RevolutRow {
    #[serde(rename = "Type")]
    txn_type: String,
    #[serde(rename = "Product")]
    product: String,
    #[serde(rename = "Started Date")]
    _started_date: String,
    #[serde(rename = "Completed Date")]
    completed_date: String,
    #[serde(rename = "Description")]
    description: String,
    #[serde(rename = "Amount")]
    amount: f64,
    #[serde(rename = "Fee")]
    _fee: f64,
    #[serde(rename = "Currency")]
    currency: String,
    #[serde(rename = "State")]
    state: String,
    #[serde(rename = "Balance")]
    _balance: f64,
}

/// Your engine transaction as in template.json (extended with optional fields
/// to allow post-checks for missing information).
#[derive(Debug, Serialize)]
pub struct EngineTransaction {
    txn_id: String,
    date: String,       // "YYYY-MM-DD"
    account_id: String, // e.g. "REVOLUT_CURRENT"
    #[serde(rename = "type")]
    txn_type: String, // "income", "expense", "internal_transfer", ...
    category: String,   // "food_groceries", "fx_exchange", ...
    amount: f64,
    currency: String,
    description: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    tags: Vec<String>,
    contra_account_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    missing_fields: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fx_rate_to_reporting: Option<f64>,
}

/// Parse the Revolut CSV and return engine transactions
pub fn parse_revolut_csv<P: AsRef<Path>>(
    path: P,
    settings: &Settings,
) -> Result<Vec<EngineTransaction>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(file);

    let mut result = Vec::new();

    for (idx, record) in rdr.deserialize::<RevolutRow>().enumerate() {
        let row = match record {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Skipping row {}: {}", idx, e);
                continue;
            }
        };

        // Only completed transactions
        if row.state.trim().to_uppercase() != "COMPLETED" {
            continue;
        }

        // Map product to account id via settings, fallback to REVOLUT_* pattern
        let product_key = row.product.trim().to_string();
        let account_id = settings
            .account_map
            .get(&product_key)
            .cloned()
            .unwrap_or_else(|| format!("REVOLUT_{}", product_key.to_uppercase().replace(' ', "_")));

        // Use completed date as transaction date, strip time
        let date = parse_completed_date_to_day(&row.completed_date)
            .unwrap_or_else(|| "1970-01-01".to_string());

        // Map Revolut type to engine type and initial category
        let (mut engine_type, mut category) = map_type_and_category(&row.txn_type);

        // Try to improve category based on description keywords when it's a card payment/expense
        if let Some(desc_cat) = infer_category_from_settings(&row.description, settings) {
            category = desc_cat.0;
        }

        // Track missing fields for post-checks
        let mut missing_fields: Vec<String> = Vec::new();
        if category == "uncategorized" || category == "other" {
            missing_fields.push("category".to_string());
        }

        // Try inferring contra account via settings; if still missing for transfers, flag
        let mut contra_account_id: Option<String> =
            infer_contra_from_settings(&row.description, settings);
        if engine_type == "internal_transfer" && contra_account_id.is_none() {
            missing_fields.push("contra_account_id".to_string());
        }

        // Build a deterministic id
        let txn_id = format!("{}-{}-{}", date, account_id, idx);

        // For now ignore row.fee, row.balance
        // Allow overriding engine txn_type based on resolved category via settings
        if let Some(override_type) = settings.category_type_map.get(&category) {
            // Only apply sane values; else keep detected type
            let allowed = [
                "income",
                "expense",
                "internal_transfer",
                "trade",
                "loan_payment",
            ];
            if allowed.contains(&override_type.as_str()) {
                engine_type = override_type.clone();
            }
        }

        let mut tx = EngineTransaction {
            txn_id,
            date,
            account_id,
            txn_type: engine_type,
            category,
            amount: row.amount,
            currency: row.currency,
            description: row.description,
            tags: Vec::new(),
            contra_account_id,
            missing_fields,
            fx_rate_to_reporting: None,
        };

        // Add default tags and rule-specific tags
        tx.tags.extend(settings.default_tags.iter().cloned());
        if let Some((_, rule_tags)) = infer_category_from_settings(&tx.description, settings) {
            tx.tags.extend(rule_tags);
        }

        result.push(tx);
    }

    Ok(result)
}

/// Parse "YYYY-MM-DD HH:MM:SS" to "YYYY-MM-DD"
fn parse_completed_date_to_day(s: &str) -> Option<String> {
    // Try full datetime first
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.date().format("%Y-%m-%d").to_string());
    }
    // Fallback to plain date
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d.format("%Y-%m-%d").to_string());
    }
    None
}

/// Map Revolut "Type" to engine type + category
fn map_type_and_category(revolut_type: &str) -> (String, String) {
    match revolut_type.trim() {
        "Card Payment" => ("expense".to_string(), "card_payment".to_string()),
        "Card Refund" => ("income".to_string(), "card_refund".to_string()),
        "Topup" => ("income".to_string(), "topup".to_string()),
        "Reward" => ("income".to_string(), "reward".to_string()),
        "Transfer" => ("internal_transfer".to_string(), "transfer".to_string()),
        "Exchange" => ("internal_transfer".to_string(), "fx_exchange".to_string()),
        _other => ("other".to_string(), "uncategorized".to_string()),
    }
}

/// Infer category and tags using settings rules. Returns (category, tags) if matched.
fn infer_category_from_settings(desc: &str, settings: &Settings) -> Option<(String, Vec<String>)> {
    let d = desc.to_lowercase();
    for rule in &settings.category_rules {
        if rule.is_regex {
            if let Ok(re) = regex::Regex::new(&rule.pattern) {
                if re.is_match(&d) {
                    return Some((rule.category.clone(), rule.tags.clone()));
                }
            }
        } else if d.contains(&rule.pattern.to_lowercase()) {
            return Some((rule.category.clone(), rule.tags.clone()));
        }
    }
    None
}

/// Infer contra account using settings rules.
fn infer_contra_from_settings(desc: &str, settings: &Settings) -> Option<String> {
    let d = desc.to_lowercase();
    for rule in &settings.contra_account_rules {
        if rule.is_regex {
            if let Ok(re) = regex::Regex::new(&rule.pattern) {
                if re.is_match(&d) {
                    return Some(rule.contra_account_id.clone());
                }
            }
        } else if d.contains(&rule.pattern.to_lowercase()) {
            return Some(rule.contra_account_id.clone());
        }
    }
    None
}

// Example: read CSV and write JSON to a file or stdout
fn main() -> Result<(), Box<dyn Error>> {
    // Usage:
    //   revolut <input_csv> [output_json] [settings_json]
    // If output_json is omitted, defaults to writing to `dashboard/dashboard.json`.
    // If output_json is set to "-", prints to stdout.

    let args: Vec<String> = std::env::args().collect();

    let input_path = if args.len() > 1 {
        &args[1]
    } else {
        "revolut.csv"
    };

    let default_output = "../../../../database/dashboard.json";
    let output_path = if args.len() > 2 {
        &args[2]
    } else {
        default_output
    };

    // Load settings: third arg or default to `settings.json` next to the binary
    let settings_path = if args.len() > 3 {
        &args[3]
    } else {
        "settings.json"
    };
    let settings = load_settings(settings_path);

    let txs = parse_revolut_csv(input_path, &settings)?;
    if output_path == "-" {
        let json = serde_json::to_string_pretty(&txs)?;
        println!("{}", json);
    } else {
        let out_path = std::path::Path::new(output_path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // If output is dashboard/dashboard.json, merge into template-style document
        let merged = if out_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|n| n.eq_ignore_ascii_case("dashboard.json"))
            .unwrap_or(false)
        {
            merge_into_dashboard(out_path, &txs)?
        } else {
            serde_json::to_string_pretty(&txs)?
        };

        std::fs::write(out_path, merged)?;
        eprintln!("Wrote Revolut transactions to {}", out_path.display());
    }

    Ok(())
}

// Merge new transactions into an existing dashboard JSON, preserving layout,
// deduplicating by txn_id, and sorting chronologically by date.
fn merge_into_dashboard(
    out_path: &std::path::Path,
    new_txs: &[EngineTransaction],
) -> Result<String, Box<dyn Error>> {
    let existing: Option<Value> = if out_path.exists() {
        let content = std::fs::read_to_string(out_path)?;
        match serde_json::from_str::<Value>(&content) {
            Ok(v) => Some(v),
            Err(_) => None,
        }
    } else {
        None
    };

    // Ensure we have an object root
    let mut root = existing.unwrap_or_else(|| Value::Object(serde_json::Map::new()));

    // Ensure "transactions" is an owned Vec<Value> we can modify safely
    let mut txs_array: Vec<Value> = match root.get("transactions") {
        Some(v) => v.as_array().cloned().unwrap_or_else(|| Vec::new()),
        None => Vec::new(),
    };

    // Build a set of existing txn_ids to dedupe
    use std::collections::HashSet;
    let mut existing_ids: HashSet<String> = HashSet::new();
    for v in txs_array.iter() {
        if let Some(id) = v.get("txn_id").and_then(|x| x.as_str()) {
            existing_ids.insert(id.to_string());
        }
    }

    // Append new transactions that are not already present
    for tx in new_txs {
        if existing_ids.contains(&tx.txn_id) {
            continue;
        }
        let v = serde_json::to_value(tx)?;
        txs_array.push(v);
    }

    // Sort transactions chronologically by `date` (YYYY-MM-DD)
    txs_array.sort_by(|a, b| {
        let ad = a.get("date").and_then(|x| x.as_str()).unwrap_or("");
        let bd = b.get("date").and_then(|x| x.as_str()).unwrap_or("");
        ad.cmp(bd)
    });

    // Write back the updated transactions array into the root
    root.as_object_mut()
        .unwrap()
        .insert("transactions".to_string(), Value::Array(txs_array));

    Ok(serde_json::to_string_pretty(&root)?)
}
