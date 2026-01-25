use anyhow::{anyhow, Context, Result};
use chrono::{NaiveDate, NaiveDateTime};
use encoding_rs::GB18030;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;

pub const PARSER_NAME: &str = "alipay";

pub struct AlipayCsvParser {
    pub account_id: String,
    pub currency: String,
    pub only_successful: bool,
}

impl AlipayCsvParser {
    pub fn new(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            currency: "CNY".to_string(),
            only_successful: true,
        }
    }

    pub fn with_currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = currency.into();
        self
    }

    pub fn with_only_successful(mut self, only_successful: bool) -> Self {
        self.only_successful = only_successful;
        self
    }

    /// Creates account entries for the Alipay accounts used by this parser.
    ///
    /// Note: Some fields cannot be determined from the CSV and are left as null.
    pub fn create_accounts(&self) -> Vec<Value> {
        vec![json!({
            "account_id": self.account_id,
            "structural_type": "bank",
            "institution": "Alipay",
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
            "notes": "Alipay wallet account - fields may need manual completion"
        })]
    }

    #[allow(dead_code)]
    pub fn create_account(&self) -> Value {
        self.create_accounts()
            .into_iter()
            .next()
            .unwrap_or_else(|| json!({}))
    }

    pub fn parse_reader<R: Read>(&self, mut reader: R) -> Result<Vec<Value>> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;

        let decoded = decode_text_lossy(&buf);

        // Strip export preamble and keep the actual CSV table
        let csv_text = slice_to_csv_table(&decoded)
            .context("Could not find Alipay CSV header row starting with '交易时间,'")?;

        let mut csv_reader = csv::ReaderBuilder::new()
            .flexible(true)
            .trim(csv::Trim::All)
            .from_reader(csv_text.as_bytes());

        let headers = csv_reader.headers().context("Missing CSV headers")?.clone();

        let idx_time = find_col(&headers, "交易时间")?;
        let idx_category = find_col(&headers, "交易分类")?;
        let idx_counterparty = find_col(&headers, "交易对方")?;
        let idx_item = find_col(&headers, "商品说明")?;
        let idx_inout = find_col(&headers, "收/支")?;
        let idx_amount = find_col(&headers, "金额")?;
        let idx_status = find_col(&headers, "交易状态")?;
        let idx_order = find_col(&headers, "交易订单号")?;
        let idx_note = find_optional_col(&headers, "备注");

        let mut out = Vec::new();

        for (row_idx, rec) in csv_reader.records().enumerate() {
            let rec = rec.with_context(|| format!("CSV read error at row {}", row_idx + 2))?;

            let status = rec.get(idx_status).unwrap_or("").trim();
            if self.only_successful && status != "交易成功" {
                continue;
            }

            let raw_dt = rec.get(idx_time).unwrap_or("").trim();
            let date = parse_alipay_datetime(raw_dt)
                .with_context(|| format!("Invalid datetime '{}' at row {}", raw_dt, row_idx + 2))?;

            let amount_raw = rec.get(idx_amount).unwrap_or("");
            let amount = parse_amount(amount_raw).with_context(|| {
                format!("Invalid amount '{}' at row {}", amount_raw, row_idx + 2)
            })?;

            let inout = rec.get(idx_inout).unwrap_or("").trim();
            let txn_type = match inout {
                "支出" => "expense",
                "收入" => "income",
                "不计收支" => "internal_transfer",
                _ => {
                    if amount < 0.0 {
                        "expense"
                    } else {
                        "income"
                    }
                }
            };

            let mut desc = rec.get(idx_item).unwrap_or("").trim().to_string();
            if desc.is_empty() {
                desc = rec.get(idx_counterparty).unwrap_or("").trim().to_string();
            }

            desc = html_escape::decode_html_entities(&desc).to_string();
            desc = desc.replace('\u{00A0}', " ");
            desc = desc.split_whitespace().collect::<Vec<_>>().join(" ");

            let cat = rec.get(idx_category).unwrap_or("").trim();
            if !cat.is_empty() {
                desc = format!("{} [{}]", desc, cat);
            }

            if let Some(i) = idx_note {
                let note = rec.get(i).unwrap_or("").trim();
                if !note.is_empty() {
                    desc = format!("{} ({})", desc, note);
                }
            }

            let order_no = rec
                .get(idx_order)
                .unwrap_or("")
                .trim()
                .trim_end_matches('\t');

            let (from_account_id, to_account_id) = match txn_type {
                "expense" => (self.account_id.clone(), "EXTERNAL_PAYEE".to_string()),
                "income" => ("EXTERNAL_PAYER".to_string(), self.account_id.clone()),
                "internal_transfer" => (self.account_id.clone(), self.account_id.clone()),
                _ => {
                    if amount < 0.0 {
                        (self.account_id.clone(), "EXTERNAL_PAYEE".to_string())
                    } else {
                        ("EXTERNAL_PAYER".to_string(), self.account_id.clone())
                    }
                }
            };

            let txn_id = make_alipay_txn_id(
                &self.account_id,
                date,
                amount,
                &self.currency,
                &desc,
                order_no,
                row_idx + 1,
            );

            out.push(json!({
                "date": date.format("%Y-%m-%d").to_string(),
                "from_account_id": from_account_id,
                "to_account_id": to_account_id,
                "type": txn_type,
                "category": "uncategorized",
                "amount": amount,
                "currency": self.currency.clone(),
                "description": desc,
                "txn_id": txn_id
            }));
        }

        Ok(out)
    }
}

/// Decode Alipay export bytes into text.
///
/// Alipay CSV exports are commonly GBK/GB18030, but sometimes can be UTF-8.
fn decode_text_lossy(bytes: &[u8]) -> String {
    // UTF-8 BOM
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);

    if let Ok(s) = std::str::from_utf8(bytes) {
        // Heuristic: if we can already see the Chinese header, accept UTF-8.
        if s.contains("交易时间") {
            return s.to_string();
        }
    }

    let (decoded, _, _) = GB18030.decode(bytes);
    decoded.into_owned()
}

fn slice_to_csv_table(decoded: &str) -> Result<String> {
    let needle = "交易时间,";
    let start = decoded
        .find(needle)
        .ok_or_else(|| anyhow!("Missing Alipay header row (expected to find '交易时间,')"))?;
    Ok(decoded[start..].to_string())
}

fn find_col(headers: &csv::StringRecord, name: &str) -> Result<usize> {
    headers
        .iter()
        .position(|h| h.trim() == name)
        .ok_or_else(|| anyhow!("Missing required column '{}'", name))
}

fn find_optional_col(headers: &csv::StringRecord, name: &str) -> Option<usize> {
    headers.iter().position(|h| h.trim() == name)
}

fn parse_alipay_datetime(raw: &str) -> Result<NaiveDate> {
    let dt = NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")?;
    Ok(dt.date())
}

fn parse_amount(raw: &str) -> Result<f64> {
    let s = raw.trim().replace(",", "");
    if s.is_empty() {
        return Err(anyhow!("empty amount"));
    }
    Ok(s.parse::<f64>()?)
}

fn make_alipay_txn_id(
    account_id: &str,
    date: NaiveDate,
    amount: f64,
    currency: &str,
    description: &str,
    order_no: &str,
    row_index: usize,
) -> String {
    let s = format!(
        "{}|{}|{:.8}|{}|{}|{}|{}",
        account_id,
        date.format("%Y-%m-%d"),
        amount,
        currency,
        description.trim(),
        order_no.trim(),
        row_index
    );

    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let hash = hasher.finalize();

    format!("ALIPAY-{}", hex::encode(&hash[..12]))
}

/// Merges Alipay transactions into an existing database.json Value.
/// Assumes database.json has a top level "transactions": [] array.
/// Automatically skips duplicate transactions based on txn_id.
pub fn merge_transactions_into_template(
    template: Value,
    new_txns: Vec<Value>,
) -> Result<(Value, utils::transactions::MergeStats)> {
    utils::merge_transactions_with_deduplication(template, new_txns)
}

/// Merges Alipay account entries into an existing database.json Value.
/// Assumes database.json has a top level "accounts": [] array.
/// Automatically skips duplicate accounts based on account_id.
pub fn merge_accounts_into_template(
    template: Value,
    new_accounts: Vec<Value>,
) -> Result<(Value, utils::accounts::MergeStats)> {
    utils::merge_accounts_with_deduplication(template, new_accounts)
}
