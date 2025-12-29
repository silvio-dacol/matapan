use anyhow::{anyhow, Context, Result};
use chrono::{NaiveDate, NaiveDateTime};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;

pub const PARSER_NAME: &str = "revolut";

pub struct RevolutCsvParser {
    pub account_id_current: String,
    pub account_id_savings: String,
    pub only_completed: bool,
}

impl RevolutCsvParser {
    pub fn new(account_id: impl Into<String>) -> Self {
        let base = account_id.into();
        Self {
            account_id_current: base.clone(),
            account_id_savings: format!("{}_SAVINGS", base.trim_end_matches("_CURRENT")),
            only_completed: true,
        }
    }

    pub fn with_only_completed(mut self, only_completed: bool) -> Self {
        self.only_completed = only_completed;
        self
    }

    /// Creates account entries for the Revolut accounts used by this parser.
    /// Returns a vector with the current account and optionally a savings account.
    /// Note: Some fields (like IBAN, BIC, account_number, owner, etc.)
    /// cannot be determined from CSV files and are left as null for manual completion.
    pub fn create_accounts(&self) -> Vec<Value> {
        vec![
            json!({
                "account_id": self.account_id_current,
                "structural_type": "bank",
                "institution": "Revolut",
                "country": null,
                "iban": null,
                "bic": null,
                "account_number": null,
                "owner": "self",
                "is_liability": false,
                "supports_positions": false,
                "opened_date": null,
                "closed_date": null,
                "is_active": true,
                "notes": "Revolut current account - some fields need manual completion"
            }),
            json!({
                "account_id": self.account_id_savings,
                "structural_type": "bank",
                "institution": "Revolut",
                "country": null,
                "iban": null,
                "bic": null,
                "account_number": null,
                "owner": "self",
                "is_liability": false,
                "supports_positions": false,
                "opened_date": null,
                "closed_date": null,
                "is_active": true,
                "notes": "Revolut savings pocket - some fields need manual completion"
            })
        ]
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
            
            // Determine account_id based on Product field
            let account_id = match row._product.as_deref() {
                Some("Savings") => &self.account_id_savings,
                _ => &self.account_id_current,
            };

            let txn_type = infer_type(amount, row.r#type.as_deref(), &description);
            
            // Determine from/to accounts
            let (from_account_id, to_account_id) = determine_accounts(
                account_id,
                &txn_type,
                amount,
                &description,
                &self.account_id_current,
                &self.account_id_savings,
            );

            let txn_id = make_txn_id(
                account_id,
                date,
                amount,
                &currency,
                &description,
                idx + 1,
            );

            let txn = json!({
                "date": date.format("%Y-%m-%d").to_string(),
                "from_account_id": from_account_id,
                "to_account_id": to_account_id,
                "type": txn_type,
                "category": "uncategorized",
                "amount": amount,
                "currency": currency,
                "description": description,
                "txn_id": txn_id
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

fn infer_type(amount: f64, revolut_type: Option<&str>, description: &str) -> String {
    let rt = revolut_type.unwrap_or("").to_lowercase();

    if rt.contains("transfer") {
        // Check if this is a transfer to/from another person (not internal)
        if description.starts_with("Transfer to ") {
            // Pattern like "Transfer to Mario Rossi" - payment to person with Revolut Account
            "expense".to_string()
        } else if description.starts_with("To ") && !description.contains("To pocket") {
            // Pattern like "To Mario Rossi" - payment to external person without Revolut
            "expense".to_string()
        } else if description.starts_with("Transfer from ") {
            // Pattern like "Transfer from Mario Rossi" - payment from person with Revolut Account
            "income".to_string()
        } else if description.starts_with("Payment from ") && !description.contains("From pocket") {
            // Pattern like "From Mario Rossi" - payment from external person without Revolut
            "income".to_string()
        } else if description.contains("To pocket") || description.contains("Pocket Withdrawal") {
            // Internal transfers between accounts/pockets
            "internal_transfer".to_string()
        } else {
            "internal_transfer".to_string()
        }
    } else if rt.contains("exchange") {
        "fx".to_string()
    } else {
        if amount < 0.0 { "expense" } else { "income" }.to_string()
    }
}

/// Determines from_account_id and to_account_id based on transaction type and details
fn determine_accounts(
    account_id: &str,
    txn_type: &str,
    amount: f64,
    description: &str,
    current_account: &str,
    savings_account: &str,
) -> (String, String) {
    match txn_type {
        "internal_transfer" => {
            // Parse pocket transfers like "To pocket SEK Cazzate from SEK"
            if description.contains("To pocket") {
                // Money moving from Current to Savings pocket
                if amount > 0.0 {
                    // Positive amount in Savings means money came FROM Current TO Savings
                    if account_id == savings_account {
                        (current_account.to_string(), savings_account.to_string())
                    } else {
                        // In Current with positive would be unusual for "To pocket"
                        (account_id.to_string(), savings_account.to_string())
                    }
                } else {
                    // Negative amount in Current means money going FROM Current TO Savings
                    if account_id == current_account {
                        (current_account.to_string(), savings_account.to_string())
                    } else {
                        // In Savings with negative would be unusual for "To pocket"
                        (savings_account.to_string(), account_id.to_string())
                    }
                }
            } else if description.contains("Pocket Withdrawal") {
                // Money moving back from Savings pocket to Current
                if amount < 0.0 {
                    // Negative amount in Savings means money leaving Savings to Current
                    if account_id == savings_account {
                        (savings_account.to_string(), current_account.to_string())
                    } else {
                        (account_id.to_string(), current_account.to_string())
                    }
                } else {
                    // Positive amount in Current means money came from Savings
                    if account_id == current_account {
                        (savings_account.to_string(), current_account.to_string())
                    } else {
                        (savings_account.to_string(), account_id.to_string())
                    }
                }
            } else {
                // Generic internal transfer
                if amount < 0.0 {
                    (account_id.to_string(), "INTERNAL_DESTINATION".to_string())
                } else {
                    ("INTERNAL_SOURCE".to_string(), account_id.to_string())
                }
            }
        }
        "expense" => {
            // Money leaving the account
            (account_id.to_string(), "EXTERNAL_PAYEE".to_string())
        }
        "income" => {
            // Money coming into the account
            ("EXTERNAL_PAYER".to_string(), account_id.to_string())
        }
        "fx" => {
            // Currency exchange - special handling
            if amount < 0.0 {
                (account_id.to_string(), "FX_EXCHANGE".to_string())
            } else {
                ("FX_EXCHANGE".to_string(), account_id.to_string())
            }
        }
        _ => {
            // Default case
            if amount < 0.0 {
                (account_id.to_string(), "EXTERNAL_PAYEE".to_string())
            } else {
                ("EXTERNAL_PAYER".to_string(), account_id.to_string())
            }
        }
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
/// Automatically skips duplicate transactions based on txn_id.
///
/// # Returns
/// * `Result<(Value, utils::transactions::MergeStats)>` - The merged database and merge statistics
pub fn merge_transactions_into_template(
    template: Value,
    new_txns: Vec<Value>,
) -> Result<(Value, utils::transactions::MergeStats)> {
    utils::merge_transactions_with_deduplication(template, new_txns)
}

/// Merges Revolut account entries into an existing database.json Value.
/// Assumes database.json has a top level "accounts": [] array.
/// Automatically skips duplicate accounts based on account_id.
///
/// # Returns
/// * `Result<(Value, utils::accounts::MergeStats)>` - The merged database and merge statistics
pub fn merge_accounts_into_template(
    template: Value,
    new_accounts: Vec<Value>,
) -> Result<(Value, utils::accounts::MergeStats)> {
    utils::merge_accounts_with_deduplication(template, new_accounts)
}
