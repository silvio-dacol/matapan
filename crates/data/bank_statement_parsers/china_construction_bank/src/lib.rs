use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook, Data, Reader, Xls};
use chrono::NaiveDate;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const PARSER_NAME: &str = "ccb";

pub struct CcbXlsParser {
    pub account_id: String,
    pub only_nonempty_rows: bool,
}

impl CcbXlsParser {
    pub fn new(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            only_nonempty_rows: true,
        }
    }

    pub fn with_only_nonempty_rows(mut self, only_nonempty_rows: bool) -> Self {
        self.only_nonempty_rows = only_nonempty_rows;
        self
    }

    /// Creates account entries for the CCB accounts used by this parser.
    ///
    /// Note: Some fields cannot be determined from the export and are left as null.
    pub fn create_accounts(&self) -> Vec<Value> {
        vec![json!({
            "account_id": self.account_id,
            "structural_type": "bank",
            "institution": "China Construction Bank",
            "country": "CN",
            "iban": null,
            "bic": null,
            "account_number": null,
            "owner": "self",
            "is_liability": false,
            "supports_positions": false,
            "opened_date": null,
            "closed_date": null,
            "is_active": true,
            "notes": "CCB checking account export (活期账户全部交易明细) - fields may need manual completion"
        })]
    }

    #[allow(dead_code)]
    pub fn create_account(&self) -> Value {
        self.create_accounts()
            .into_iter()
            .next()
            .unwrap_or_else(|| json!({}))
    }

    /// Parse a single .xls file path and return standard transaction JSON objects.
    pub fn parse_file(&self, xls_path: &str) -> Result<Vec<Value>> {
        let mut workbook: Xls<_> =
            open_workbook(xls_path).with_context(|| format!("Cannot open {}", xls_path))?;

        let sheet_name = workbook
            .sheet_names()
            .get(0)
            .cloned()
            .ok_or_else(|| anyhow!("No sheets found in {}", xls_path))?;

        let range = workbook
            .worksheet_range(&sheet_name)
            .with_context(|| format!("Cannot read sheet '{}' in {}", sheet_name, xls_path))?;

        let (header_row_idx, header_map) = find_header_row_ccb(&range).context(
            "Could not find CCB header row starting with '序号' and containing '交易日期'",
        )?;

        let c_seq = *header_map.get("序号").context("Missing column: 序号")?;
        let c_summary = *header_map.get("摘要").context("Missing column: 摘要")?;
        let c_currency = *header_map.get("币别").context("Missing column: 币别")?;
        let c_date = *header_map
            .get("交易日期")
            .context("Missing column: 交易日期")?;
        let c_amount = *header_map
            .get("交易金额")
            .context("Missing column: 交易金额")?;
        let c_location = header_map.get("交易地点/附言").copied();
        let c_counterparty = header_map.get("对方账号与户名").copied();

        let mut out = Vec::new();

        for (row_idx, row) in range.rows().enumerate().skip(header_row_idx + 1) {
            let seq_s = cell_str(row.get(c_seq)).trim().to_string();
            let summary = cell_str(row.get(c_summary)).trim().to_string();
            let date_raw = cell_str(row.get(c_date)).trim().to_string();
            let amount_raw = cell_str(row.get(c_amount)).trim().to_string();

            // Stop / skip empty tail rows
            let is_empty_row = seq_s.is_empty()
                && summary.is_empty()
                && date_raw.is_empty()
                && amount_raw.is_empty();
            if is_empty_row {
                if self.only_nonempty_rows {
                    continue;
                } else {
                    break;
                }
            }

            if date_raw.is_empty() || amount_raw.is_empty() {
                continue;
            }

            let date = parse_ccb_date(&date_raw)
                .with_context(|| format!("Invalid date '{}' in {}", date_raw, xls_path))?;

            let amount_signed = parse_ccb_amount(&amount_raw)
                .with_context(|| format!("Invalid amount '{}' in {}", amount_raw, xls_path))?;

            // Determine direction from the original signed amount,
            // but store normalized positive amount in the database.
            let txn_type = if amount_signed < 0.0 { "expense" } else { "income" };
            let amount = amount_signed.abs();

            let currency_raw = cell_str(row.get(c_currency)).trim().to_string();
            let currency = normalize_ccb_currency(&currency_raw);

            let location = c_location
                .map(|idx| cell_str(row.get(idx)))
                .unwrap_or_default()
                .trim()
                .to_string();

            let counterparty = c_counterparty
                .map(|idx| cell_str(row.get(idx)))
                .unwrap_or_default()
                .trim()
                .to_string();

            let mut desc_parts = Vec::new();
            if !summary.is_empty() {
                desc_parts.push(summary.clone());
            }
            if !location.is_empty() && location != "***" {
                desc_parts.push(location.clone());
            }
            if !counterparty.is_empty() {
                desc_parts.push(counterparty.clone());
            }

            let description = if desc_parts.is_empty() {
                "CCB transaction".to_string()
            } else {
                desc_parts.join(" ")
            };

            let (from_account_id, to_account_id) = if txn_type == "expense" {
                (self.account_id.clone(), "EXTERNAL_PAYEE".to_string())
            } else {
                ("EXTERNAL_PAYER".to_string(), self.account_id.clone())
            };

            let txn_id = make_ccb_txn_id(
                &self.account_id,
                date,
                amount,
                currency,
                &description,
                &seq_s,
                row_idx + 1,
            );

            out.push(json!({
                "date": date.format("%Y-%m-%d").to_string(),
                "from_account_id": from_account_id,
                "to_account_id": to_account_id,
                "type": txn_type,
                "category": "uncategorized",
                "amount": amount,
                "currency": currency,
                "description": description,
                "txn_id": txn_id
            }));
        }

        Ok(out)
    }
}

fn find_header_row_ccb(range: &calamine::Range<Data>) -> Result<(usize, HashMap<String, usize>)> {
    for (r_idx, row) in range.rows().enumerate() {
        let first = cell_str(row.get(0)).trim().to_string();
        if first == "序号" {
            let mut map = HashMap::new();
            for (c_idx, cell) in row.iter().enumerate() {
                let name = cell_str(Some(cell)).trim().to_string();
                if !name.is_empty() {
                    map.insert(name, c_idx);
                }
            }
            // sanity check
            if map.contains_key("交易日期") && map.contains_key("交易金额") {
                return Ok((r_idx, map));
            }
        }
    }
    Err(anyhow!("Header row not found"))
}

fn cell_str(cell: Option<&Data>) -> String {
    let Some(c) = cell else {
        return String::new();
    };

    match c {
        Data::String(s) => s.clone(),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::Empty => String::new(),
        _ => c.to_string(),
    }
}

fn parse_ccb_date(raw: &str) -> Result<NaiveDate> {
    let s = raw.trim();

    // Most common in your file: 20241221
    if s.len() == 8 && s.chars().all(|c| c.is_ascii_digit()) {
        let y: i32 = s[0..4].parse()?;
        let m: u32 = s[4..6].parse()?;
        let d: u32 = s[6..8].parse()?;
        return NaiveDate::from_ymd_opt(y, m, d).ok_or_else(|| anyhow!("invalid ymd"));
    }

    // Sometimes exports might use yyyy-mm-dd
    if let Ok(dt) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(dt);
    }

    Err(anyhow!("unsupported date format: {}", s))
}

fn parse_ccb_amount(raw: &str) -> Result<f64> {
    let mut s = raw.trim().to_string();

    // Remove commas used as thousand separators
    s = s.replace(",", "");

    // Sometimes there can be trailing tabs or spaces
    s = s.trim().to_string();

    if s.is_empty() {
        return Err(anyhow!("empty amount"));
    }
    Ok(s.parse::<f64>()?)
}

fn normalize_ccb_currency(currency_raw: &str) -> &'static str {
    // In your file, the column contains "人民币元"
    if currency_raw.contains("人民币") {
        "CNY"
    } else {
        // Fallback: assume CNY unless you later want to map other CCB currency values
        "CNY"
    }
}

fn make_ccb_txn_id(
    account_id: &str,
    date: NaiveDate,
    amount: f64,
    currency: &str,
    description: &str,
    seq: &str,
    row_index: usize,
) -> String {
    let s = format!(
        "{}|{}|{:.8}|{}|{}|{}|{}",
        account_id,
        date.format("%Y-%m-%d"),
        amount,
        currency,
        description.trim(),
        seq.trim(),
        row_index
    );

    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let hash = hasher.finalize();

    format!("CCB-{}", hex::encode(&hash[..12]))
}

/// Merges CCB transactions into an existing database.json Value.
/// Assumes database.json has a top level "transactions": [] array.
/// Automatically skips duplicate transactions based on txn_id.
pub fn merge_transactions_into_template(
    template: Value,
    new_txns: Vec<Value>,
) -> Result<(Value, utils::transactions::MergeStats)> {
    utils::merge_transactions_with_deduplication(template, new_txns)
}

/// Merges CCB account entries into an existing database.json Value.
/// Assumes database.json has a top level "accounts": [] array.
/// Automatically skips duplicate accounts based on account_id.
pub fn merge_accounts_into_template(
    template: Value,
    new_accounts: Vec<Value>,
) -> Result<(Value, utils::accounts::MergeStats)> {
    utils::merge_accounts_with_deduplication(template, new_accounts)
}
