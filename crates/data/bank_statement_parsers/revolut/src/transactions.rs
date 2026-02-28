//! Parses Revolut CSV rows into normalized transactions and used-account metadata.

use anyhow::{anyhow, Context, Result};
use chrono::{NaiveDate, NaiveDateTime};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Read;
use utils::{build_transaction, TransactionInput};

use crate::RevolutCsvParser;

#[derive(Debug, Deserialize)]
struct RevolutRow {
    #[serde(rename = "Type")]
    revolut_type: Option<String>,

    #[serde(rename = "Product")]
    product: Option<String>,

    #[serde(rename = "Started Date")]
    started_date: Option<String>,

    #[serde(rename = "Completed Date")]
    completed_date: Option<String>,

    #[serde(rename = "Description")]
    description: Option<String>,

    #[serde(rename = "Amount")]
    amount: f64,

    #[serde(rename = "Fee")]
    fee: Option<f64>,

    #[serde(rename = "Currency")]
    currency: String,

    #[serde(rename = "State")]
    state: Option<String>,

    #[serde(rename = "Balance")]
    balance: Option<f64>,
}

pub fn parse_transactions<R: Read>(
    parser: &RevolutCsvParser,
    reader: R,
) -> Result<(Vec<Value>, Vec<String>)> {
    let mut csv_reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(reader);

    let mut out: Vec<Value> = Vec::new();
    let mut used_accounts = HashSet::new();

    for (idx, row) in csv_reader.deserialize::<RevolutRow>().enumerate() {
        let row = row.with_context(|| format!("CSV deserialize error at row {}", idx + 1))?;

        if parser.only_completed {
            let state = row.state.as_deref().unwrap_or("");
            if state != "COMPLETED" {
                continue;
            }
        }

        let date = field_date(&row).with_context(|| format!("Invalid date at row {}", idx + 1))?;
        let description = field_description(&row);
        let currency = field_currency(&row);
        let amount = field_amount(&row);
        let account_id = field_account_id(parser, &row);
        let txn_type = field_type(&row, amount, &description);
        let (from_account_id, to_account_id) =
            field_accounts(parser, &account_id, &txn_type, amount, &description);

        if from_account_id == parser.account_id_current
            || to_account_id == parser.account_id_current
        {
            used_accounts.insert(parser.account_id_current.clone());
        }
        if from_account_id == parser.account_id_savings
            || to_account_id == parser.account_id_savings
        {
            used_accounts.insert(parser.account_id_savings.clone());
        }

        let txn_id = field_txn_id(&account_id, date, amount, &currency, &description, idx + 1);

        out.push(build_transaction(&TransactionInput {
            date: date.format("%Y-%m-%d").to_string(),
            from_account_id,
            to_account_id,
            transaction_type: txn_type,
            category: field_category(),
            amount: amount.abs(),
            currency: currency.clone(),
            description: description.clone(),
            description_en: field_description_en(),
            txn_id,
        }));

        if let Some(fee) = field_fee(&row) {
            if fee != 0.0 {
                let fee_description = format!("Fees: {}", description);
                let fee_txn_id = field_txn_id(
                    &account_id,
                    date,
                    fee,
                    &currency,
                    &format!("FEE|{}", description),
                    idx + 1,
                );

                out.push(build_transaction(&TransactionInput {
                    date: date.format("%Y-%m-%d").to_string(),
                    from_account_id: account_id,
                    to_account_id: "EXTERNAL_PAYEE".to_string(),
                    transaction_type: "expense".to_string(),
                    category: field_category(),
                    amount: fee.abs(),
                    currency,
                    description: fee_description,
                    description_en: field_description_en(),
                    txn_id: fee_txn_id,
                }));
            }
        }

        let _ = &row.balance;
    }

    Ok((out, used_accounts.into_iter().collect()))
}

fn field_date(row: &RevolutRow) -> Result<NaiveDate> {
    let raw = row
        .completed_date
        .as_ref()
        .or(row.started_date.as_ref())
        .ok_or_else(|| anyhow!("Missing date"))?;

    if let Ok(dt) = NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S") {
        return Ok(dt.date());
    }

    if let Ok(d) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return Ok(d);
    }

    Err(anyhow!("Unrecognized date format: {}", raw))
}

fn field_account_id(parser: &RevolutCsvParser, row: &RevolutRow) -> String {
    match row.product.as_deref() {
        Some("Savings") => parser.account_id_savings.clone(),
        _ => parser.account_id_current.clone(),
    }
}

fn field_type(row: &RevolutRow, amount: f64, description: &str) -> String {
    infer_type(amount, row.revolut_type.as_deref(), description)
}

fn field_category() -> String {
    "uncategorized".to_string()
}

fn field_amount(row: &RevolutRow) -> f64 {
    row.amount
}

fn field_currency(row: &RevolutRow) -> String {
    row.currency.clone()
}

fn field_description(row: &RevolutRow) -> String {
    row.description.clone().unwrap_or_default()
}

fn field_description_en() -> Option<String> {
    None
}

fn field_fee(row: &RevolutRow) -> Option<f64> {
    row.fee
}

fn field_txn_id(
    account_id: &str,
    date: NaiveDate,
    amount: f64,
    currency: &str,
    description: &str,
    row_index: usize,
) -> String {
    let seed = format!(
        "{}|{}|{:.8}|{}|{}|{}",
        account_id,
        date.format("%Y-%m-%d"),
        amount,
        currency,
        description.trim(),
        row_index
    );

    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let hash = hasher.finalize();

    format!("REVOLUT-{}", hex::encode(&hash[..12]))
}

fn field_accounts(
    parser: &RevolutCsvParser,
    account_id: &str,
    txn_type: &str,
    amount: f64,
    description: &str,
) -> (String, String) {
    determine_accounts(
        account_id,
        txn_type,
        amount,
        description,
        &parser.account_id_current,
        &parser.account_id_savings,
    )
}

fn infer_type(amount: f64, revolut_type: Option<&str>, description: &str) -> String {
    let rt = revolut_type.unwrap_or("").to_lowercase();

    if rt.contains("transfer") {
        if description.starts_with("Transfer to ") {
            "expense".to_string()
        } else if description.starts_with("To ") && !description.contains("To pocket") {
            "expense".to_string()
        } else if description.starts_with("Transfer from ") {
            "income".to_string()
        } else if description.starts_with("Payment from ") && !description.contains("From pocket") {
            "income".to_string()
        } else if description.contains("To pocket") || description.contains("Pocket Withdrawal") {
            "internal_transfer".to_string()
        } else if amount < 0.0 {
            "expense".to_string()
        } else {
            "income".to_string()
        }
    } else if rt.contains("exchange") {
        if amount < 0.0 {
            "expense".to_string()
        } else {
            "income".to_string()
        }
    } else if amount < 0.0 {
        "expense".to_string()
    } else {
        "income".to_string()
    }
}

fn determine_accounts(
    account_id: &str,
    txn_type: &str,
    amount: f64,
    description: &str,
    current_account: &str,
    savings_account: &str,
) -> (String, String) {
    if description.to_lowercase().contains("exchanged") {
        return (account_id.to_string(), account_id.to_string());
    }

    match txn_type {
        "internal_transfer" => {
            if description.contains("To pocket") {
                if amount > 0.0 {
                    if account_id == savings_account {
                        (current_account.to_string(), savings_account.to_string())
                    } else {
                        (account_id.to_string(), savings_account.to_string())
                    }
                } else if account_id == current_account {
                    (current_account.to_string(), savings_account.to_string())
                } else {
                    (savings_account.to_string(), account_id.to_string())
                }
            } else if description.contains("Pocket Withdrawal") {
                if amount < 0.0 {
                    if account_id == savings_account {
                        (savings_account.to_string(), current_account.to_string())
                    } else {
                        (account_id.to_string(), current_account.to_string())
                    }
                } else if account_id == current_account {
                    (savings_account.to_string(), current_account.to_string())
                } else {
                    (savings_account.to_string(), account_id.to_string())
                }
            } else if amount < 0.0 {
                (account_id.to_string(), "INTERNAL_DESTINATION".to_string())
            } else {
                ("INTERNAL_SOURCE".to_string(), account_id.to_string())
            }
        }
        "expense" => (account_id.to_string(), "EXTERNAL_PAYEE".to_string()),
        "income" => ("EXTERNAL_PAYER".to_string(), account_id.to_string()),
        "fx" => {
            if amount < 0.0 {
                (account_id.to_string(), "FX_EXCHANGE".to_string())
            } else {
                ("FX_EXCHANGE".to_string(), account_id.to_string())
            }
        }
        _ => {
            if amount < 0.0 {
                (account_id.to_string(), "EXTERNAL_PAYEE".to_string())
            } else {
                ("EXTERNAL_PAYER".to_string(), account_id.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RevolutCsvParser;

    #[test]
    fn parse_exchange_rows_as_directional_flows() {
        let csv = "Type,Product,Started Date,Completed Date,Description,Amount,Fee,Currency,State,Balance\n\
Exchange,Savings,2026-01-04 14:57:50,2026-01-04 14:57:50,Exchanged to EUR,-2510.31,25.10,SEK,COMPLETED,7414.49\n\
Exchange,Current,2026-01-04 14:57:50,2026-01-04 14:57:50,Exchanged to EUR,231.50,0.00,EUR,COMPLETED,251.81\n";

        let parser = RevolutCsvParser::new("REVOLUT");
        let (txns, _used_accounts) = parse_transactions(&parser, csv.as_bytes()).unwrap();

        assert_eq!(txns.len(), 3);

        let exchanged_out = &txns[0];
        assert_eq!(
            exchanged_out.get("type").and_then(|v| v.as_str()),
            Some("expense")
        );
        assert_eq!(
            exchanged_out
                .get("from_account_id")
                .and_then(|v| v.as_str()),
            Some("REVOLUT_SAVINGS")
        );
        assert_eq!(
            exchanged_out.get("to_account_id").and_then(|v| v.as_str()),
            Some("REVOLUT_SAVINGS")
        );
        assert_eq!(
            exchanged_out.get("amount").and_then(|v| v.as_f64()),
            Some(2510.31)
        );

        let exchanged_in = &txns[2];
        assert_eq!(
            exchanged_in.get("type").and_then(|v| v.as_str()),
            Some("income")
        );
        assert_eq!(
            exchanged_in.get("from_account_id").and_then(|v| v.as_str()),
            Some("REVOLUT_CURRENT")
        );
        assert_eq!(
            exchanged_in.get("to_account_id").and_then(|v| v.as_str()),
            Some("REVOLUT_CURRENT")
        );
        assert_eq!(
            exchanged_in.get("amount").and_then(|v| v.as_f64()),
            Some(231.5)
        );
    }
}
