use anyhow::{anyhow, Context, Result};
use chrono::{NaiveDate, NaiveDateTime};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;

pub const PARSER_NAME: &str = "revolut";

pub struct RevolutCsvParser {
    pub account_id: String,
    pub only_completed: bool,
}

impl RevolutCsvParser {
    pub fn new(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            only_completed: true,
        }
    }

    pub fn with_only_completed(mut self, only_completed: bool) -> Self {
        self.only_completed = only_completed;
        self
    }

    pub fn parse_reader<R: Read>(&self, reader: R) -> Result<Vec<Value>> {
        let mut csv_reader = csv::ReaderBuilder::new()
            .flexible(true)
            .trim(csv::Trim::All)
            .from_reader(reader);

        let mut out: Vec<Value> = Vec::new();

        for (idx, row) in csv_reader.deserialize::<RevolutRow>().enumerate() {
            let row = row.with_context(|| format!("CSV deserialize error at row {}", idx + 1))?;

            if self.only_completed {
                let state = row.state.as_deref().unwrap_or("");
                if state != "COMPLETED" {
                    continue;
                }
            }

            let date = pick_date(&row.started_date, &row.completed_date)
                .with_context(|| format!("Invalid date at row {}", idx + 1))?;

            let description = row.description.clone().unwrap_or_else(|| "".to_string());
            let currency = row.currency.clone();
            let amount = row.amount;

            let txn_type = infer_type(amount, row.r#type.as_deref());

            let txn_id = make_txn_id(
                &self.account_id,
                date,
                amount,
                &currency,
                &description,
                idx + 1,
            );

            let txn = json!({
                "txn_id": txn_id,
                "date": date.format("%Y-%m-%d").to_string(),
                "account_id": self.account_id,
                "type": txn_type,
                "category": "uncategorized",
                "amount": amount,
                "currency": currency,
                "description": description,
                "tags": [PARSER_NAME]
            });

            out.push(txn);
        }

        Ok(out)
    }
}

#[derive(Debug, Deserialize)]
struct RevolutRow {
    #[serde(rename = "Type")]
    r#type: Option<String>,

    #[serde(rename = "Product")]
    _product: Option<String>,

    #[serde(rename = "Started Date")]
    started_date: Option<String>,

    #[serde(rename = "Completed Date")]
    completed_date: Option<String>,

    #[serde(rename = "Description")]
    description: Option<String>,

    #[serde(rename = "Amount")]
    amount: f64,

    #[serde(rename = "Fee")]
    _fee: Option<f64>,

    #[serde(rename = "Currency")]
    currency: String,

    #[serde(rename = "State")]
    state: Option<String>,

    #[serde(rename = "Balance")]
    _balance: Option<f64>,
}

fn pick_date(started: &Option<String>, completed: &Option<String>) -> Result<NaiveDate> {
    let raw = completed
        .as_ref()
        .or(started.as_ref())
        .ok_or_else(|| anyhow!("Missing date"))?;

    if let Ok(dt) = NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S") {
        return Ok(dt.date());
    }

    if let Ok(d) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return Ok(d);
    }

    Err(anyhow!("Unrecognized date format: {}", raw))
}

fn infer_type(amount: f64, revolut_type: Option<&str>) -> String {
    let rt = revolut_type.unwrap_or("").to_lowercase();

    if rt.contains("transfer") {
        "internal_transfer".to_string()
    } else if rt.contains("exchange") {
        "fx".to_string()
    } else {
        if amount < 0.0 { "expense" } else { "income" }.to_string()
    }
}

fn make_txn_id(
    account_id: &str,
    date: NaiveDate,
    amount: f64,
    currency: &str,
    description: &str,
    row_index: usize,
) -> String {
    let s = format!(
        "{}|{}|{:.8}|{}|{}|{}",
        account_id,
        date.format("%Y-%m-%d"),
        amount,
        currency,
        description.trim(),
        row_index
    );

    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let hash = hasher.finalize();

    format!("REVOLUT-{}", hex::encode(&hash[..12]))
}

/// Merges Revolut transactions into an existing database.json Value.
/// Assumes database.json has a top level "transactions": [] array.
pub fn merge_transactions_into_template(
    mut template: Value,
    new_txns: Vec<Value>,
) -> Result<Value> {
    let arr = template
        .get_mut("transactions")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("database.json missing 'transactions' array"))?;

    arr.extend(new_txns);
    Ok(template)
}
