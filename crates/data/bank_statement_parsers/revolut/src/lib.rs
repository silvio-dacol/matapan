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

    // If the existing file is a bare array (legacy format), wrap it into an object
    // under the "transactions" key and ensure other sections exist.
    if root.is_array() {
        let arr = root.as_array().cloned().unwrap_or_default();
        let mut map = serde_json::Map::new();
        map.insert("transactions".to_string(), Value::Array(arr));
        map.insert("instruments".to_string(), Value::Array(Vec::new()));
        map.insert("accounts".to_string(), Value::Array(Vec::new()));
        map.insert("positions".to_string(), Value::Array(Vec::new()));
        map.insert("month_end_snapshots".to_string(), Value::Array(Vec::new()));
        root = Value::Object(map);
    }

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

    // Ensure sections exist and write back transactions
    let obj = root.as_object_mut().unwrap();
    obj.insert("transactions".to_string(), Value::Array(txs_array));

    // Ensure instruments/accounts/positions arrays exist if missing
    obj.entry("instruments")
        .or_insert_with(|| Value::Array(Vec::new()));
    obj.entry("accounts")
        .or_insert_with(|| Value::Array(Vec::new()));
    obj.entry("positions")
        .or_insert_with(|| Value::Array(Vec::new()));
    obj.entry("month_end_snapshots")
        .or_insert_with(|| Value::Array(Vec::new()));

    // Auto-create missing accounts based on transactions and contra accounts
    let mut existing_accounts: HashMap<String, Value> = obj
        .get("accounts")
        .and_then(|v| v.as_array())
        .unwrap_or(&Vec::new())
        .iter()
        .filter_map(|a| {
            let id = a.get("account_id").and_then(|x| x.as_str())?.to_string();
            Some((id, a.clone()))
        })
        .collect();

    // Gather candidate account_ids and their observed attributes
    #[derive(Default)]
    struct Observed {
        currencies: HashSet<String>,
        last_currency: Option<String>,
    }
    let mut observed: HashMap<String, Observed> = HashMap::new();

    for tx in new_txs {
        let ent = observed.entry(tx.account_id.clone()).or_default();
        ent.currencies.insert(tx.currency.clone());
        ent.last_currency = Some(tx.currency.clone());
        if let Some(ca) = &tx.contra_account_id {
            let entc = observed.entry(ca.clone()).or_default();
            entc.currencies.insert(tx.currency.clone());
            entc.last_currency = Some(tx.currency.clone());
        }
    }

    // Create missing account entries
    for (acc_id, obs) in observed.into_iter() {
        if existing_accounts.contains_key(&acc_id) {
            continue;
        }

        // Heuristic defaults for Revolut-generated accounts
        let account_type = if acc_id.to_uppercase().contains("SAVINGS") {
            "bank_savings"
        } else if acc_id.to_uppercase().contains("CREDIT_CARD") {
            "credit_card"
        } else {
            "bank_current"
        };

        let currency = obs.last_currency.unwrap_or_else(|| "SEK".to_string());

        let mut account_obj = serde_json::Map::new();
        account_obj.insert("account_id".to_string(), Value::String(acc_id.clone()));
        account_obj.insert("type".to_string(), Value::String(account_type.to_string()));
        account_obj.insert(
            "institution".to_string(),
            Value::String("Revolut".to_string()),
        );
        account_obj.insert("country".to_string(), Value::String("GB".to_string()));
        account_obj.insert("currency".to_string(), Value::String(currency));
        account_obj.insert("is_liability".to_string(), Value::Bool(false));

        existing_accounts.insert(acc_id.clone(), Value::Object(account_obj));
    }

    // Write back accounts list
    let accounts_vec: Vec<Value> = existing_accounts.into_values().collect();
    obj.insert("accounts".to_string(), Value::Array(accounts_vec));

    Ok(serde_json::to_string_pretty(&root)?)
}
