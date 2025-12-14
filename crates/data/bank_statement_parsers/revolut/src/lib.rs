use chrono::{NaiveDate, NaiveDateTime};
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::path::Path;

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub account_map: HashMap<String, String>,
    #[serde(default)]
    pub category_rules: Vec<CategoryRule>,
    #[serde(default)]
    pub category_type_map: HashMap<String, String>,
    #[serde(default)]
    pub contra_account_rules: Vec<ContraAccountRule>,
}

#[derive(Debug, Deserialize)]
pub struct CategoryRule {
    pub pattern: String,
    #[serde(default)]
    pub is_regex: bool,
    pub category: String,
}

#[derive(Debug, Deserialize)]
pub struct ContraAccountRule {
    pub pattern: String,
    #[serde(default)]
    pub is_regex: bool,
    pub contra_account_id: String,
}

pub fn load_settings<P: AsRef<Path>>(path: P) -> Settings {
    let content = std::fs::read_to_string(path);
    match content {
        Ok(c) => serde_json::from_str::<Settings>(&c).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

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

#[derive(Debug, Serialize)]
pub struct EngineTransaction {
    pub txn_id: String,
    pub date: String,
    pub account_id: String,
    #[serde(rename = "type")]
    pub txn_type: String,
    pub category: String,
    pub amount: f64,
    pub currency: String,
    pub description: String,
    pub contra_account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fx_rate_to_reporting: Option<f64>,
}

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

        if row.state.trim().to_uppercase() != "COMPLETED" {
            continue;
        }

        let product_key = row.product.trim().to_string();
        let account_id = settings
            .account_map
            .get(&product_key)
            .cloned()
            .unwrap_or_else(|| format!("REVOLUT_{}", product_key.to_uppercase().replace(' ', "_")));

        let date = parse_completed_date_to_day(&row.completed_date)
            .unwrap_or_else(|| "1970-01-01".to_string());

        let (mut engine_type, mut category) = map_type_and_category(&row.txn_type);

        if let Some(desc_cat) = infer_category_from_settings(&row.description, settings) {
            category = desc_cat;
        }

        let contra_account_id: Option<String> =
            infer_contra_from_settings(&row.description, settings);

        if let Some(override_type) = settings.category_type_map.get(&category) {
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

        let txn_id = format!("{}-{}-{}", date, account_id, idx);

        let tx = EngineTransaction {
            txn_id,
            date,
            account_id,
            txn_type: engine_type,
            category,
            amount: row.amount,
            currency: row.currency,
            description: row.description,
            contra_account_id,
            fx_rate_to_reporting: None,
        };

        result.push(tx);
    }

    Ok(result)
}

fn parse_completed_date_to_day(s: &str) -> Option<String> {
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.date().format("%Y-%m-%d").to_string());
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d.format("%Y-%m-%d").to_string());
    }
    None
}

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

fn infer_category_from_settings(desc: &str, settings: &Settings) -> Option<String> {
    let d = desc.to_lowercase();
    for rule in &settings.category_rules {
        if rule.is_regex {
            if let Ok(re) = regex::Regex::new(&rule.pattern) {
                if re.is_match(&d) {
                    return Some(rule.category.clone());
                }
            }
        } else if d.contains(&rule.pattern.to_lowercase()) {
            return Some(rule.category.clone());
        }
    }
    None
}

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

pub fn merge_into_dashboard(
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

    let mut root = existing.unwrap_or_else(|| Value::Object(serde_json::Map::new()));

    let mut txs_array: Vec<Value> = match root.get("transactions") {
        Some(v) => v.as_array().cloned().unwrap_or_else(|| Vec::new()),
        None => Vec::new(),
    };

    use std::collections::HashSet;
    let mut existing_ids: HashSet<String> = HashSet::new();
    for v in txs_array.iter() {
        if let Some(id) = v.get("txn_id").and_then(|x| x.as_str()) {
            existing_ids.insert(id.to_string());
        }
    }

    for tx in new_txs {
        if existing_ids.contains(&tx.txn_id) {
            continue;
        }
        let v = serde_json::to_value(tx)?;
        txs_array.push(v);
    }

    txs_array.sort_by(|a, b| {
        let ad = a.get("date").and_then(|x| x.as_str()).unwrap_or("");
        let bd = b.get("date").and_then(|x| x.as_str()).unwrap_or("");
        ad.cmp(bd)
    });

    // Write back transactions
    root.as_object_mut()
        .unwrap()
        .insert("transactions".to_string(), Value::Array(txs_array));

    Ok(serde_json::to_string_pretty(&root)?)
}
