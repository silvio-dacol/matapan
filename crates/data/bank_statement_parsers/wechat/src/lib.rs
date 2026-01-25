use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook, Data, Reader, Xlsx};
use chrono::{NaiveDate, NaiveDateTime};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const PARSER_NAME: &str = "wechat";

pub struct WeChatXlsxParser {
    pub account_id: String,
    pub currency: String,
    pub only_successful: bool,
}

impl WeChatXlsxParser {
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

    /// Creates account entries for the WeChat Pay accounts used by this parser.
    ///
    /// Note: Some fields cannot be determined from the export and are left as null.
    pub fn create_accounts(&self) -> Vec<Value> {
        vec![json!({
            "account_id": self.account_id,
            "structural_type": "bank",
            "institution": "WeChat Pay",
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
            "notes": "WeChat Pay wallet account - fields may need manual completion"
        })]
    }

    #[allow(dead_code)]
    pub fn create_account(&self) -> Value {
        self.create_accounts()
            .into_iter()
            .next()
            .unwrap_or_else(|| json!({}))
    }

    /// Parse a single .xlsx file path and return standard transaction JSON objects.
    pub fn parse_file(&self, xlsx_path: &str) -> Result<Vec<Value>> {
        let mut workbook: Xlsx<_> =
            open_workbook(xlsx_path).with_context(|| format!("Cannot open {}", xlsx_path))?;

        let sheet_name = workbook
            .sheet_names()
            .get(0)
            .cloned()
            .ok_or_else(|| anyhow!("No sheets found in {}", xlsx_path))?;

        let range = workbook
            .worksheet_range(&sheet_name)
            .with_context(|| format!("Cannot read sheet '{}' in {}", sheet_name, xlsx_path))?;

        // Find header row (交易时间 ...)
        let (header_row_idx, header_map) = find_header_row(&range)
            .context("Could not find WeChat header row starting with '交易时间'")?;

        // Required columns
        let c_time = *header_map
            .get("交易时间")
            .context("Missing column: 交易时间")?;
        let c_type = *header_map
            .get("交易类型")
            .context("Missing column: 交易类型")?;
        let c_counterparty = *header_map
            .get("交易对方")
            .context("Missing column: 交易对方")?;
        let c_product = *header_map.get("商品").context("Missing column: 商品")?;
        let c_inout = *header_map.get("收/支").context("Missing column: 收/支")?;
        let c_amount = *header_map
            .get("金额(元)")
            .context("Missing column: 金额(元)")?;
        let c_paymethod = *header_map
            .get("支付方式")
            .context("Missing column: 支付方式")?;
        let c_status = *header_map
            .get("当前状态")
            .context("Missing column: 当前状态")?;
        let c_trade_no = *header_map
            .get("交易单号")
            .context("Missing column: 交易单号")?;
        let c_note = header_map.get("备注").copied();

        let mut out = Vec::new();

        for (i, row) in range.rows().enumerate().skip(header_row_idx + 1) {
            let dt_s = cell_str(row.get(c_time));
            let dt_s = dt_s.trim().to_string();
            if dt_s.is_empty() {
                // WeChat exports typically end with empty rows after data
                continue;
            }

            // Status filter (only_successful)
            let status = cell_str(row.get(c_status));
            let status = status.trim().to_string();
            if self.only_successful {
                // Keep the common successful states
                // Examples: 支付成功, 对方已收钱, 充值完成
                // Also WeChat exports sometimes include refund states; we keep those too (still real movements).
                let is_ok = status.contains("成功")
                    || status.contains("已收钱")
                    || status.contains("完成")
                    || status.contains("退款");
                if !is_ok {
                    continue;
                }
            }

            let date = parse_wechat_datetime(&dt_s)
                .with_context(|| format!("Invalid datetime '{}' in {}", dt_s, xlsx_path))?;

            let amount_raw = cell_str(row.get(c_amount));
            let amount = parse_wechat_amount(&amount_raw)
                .with_context(|| format!("Invalid amount '{}' in {}", amount_raw, xlsx_path))?;

            let inout = cell_str(row.get(c_inout)).trim().to_string();
            let wx_type = cell_str(row.get(c_type)).trim().to_string();

            // Determine txn_type
            let txn_type = classify_txn_type(&inout, &wx_type, amount);

            let counterparty = cell_str(row.get(c_counterparty)).trim().to_string();
            let product = cell_str(row.get(c_product)).trim().to_string();
            let paymethod = cell_str(row.get(c_paymethod)).trim().to_string();
            let note = c_note
                .map(|idx| cell_str(row.get(idx)))
                .unwrap_or_default()
                .trim()
                .to_string();

            let mut desc_parts = Vec::new();
            if !counterparty.is_empty() && counterparty != "/" {
                desc_parts.push(counterparty);
            }
            if !product.is_empty() && product != "/" {
                desc_parts.push(product);
            }
            if !wx_type.is_empty() && wx_type != "/" {
                desc_parts.push(format!("[{}]", wx_type));
            }
            if !paymethod.is_empty() && paymethod != "/" {
                desc_parts.push(format!("({})", paymethod));
            }
            if !note.is_empty() && note != "/" {
                desc_parts.push(format!("note: {}", note));
            }

            let description = if desc_parts.is_empty() {
                "WeChat Pay transaction".to_string()
            } else {
                desc_parts.join(" ")
            };

            let trade_no = cell_str(row.get(c_trade_no)).trim().to_string();

            let (from_account_id, to_account_id) = match txn_type.as_str() {
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

            let txn_id = make_wechat_txn_id(
                &self.account_id,
                &dt_s, // include time for uniqueness
                amount,
                &self.currency,
                &description,
                &trade_no,
                i + 1,
            );

            out.push(json!({
                "date": date.format("%Y-%m-%d").to_string(),
                "from_account_id": from_account_id,
                "to_account_id": to_account_id,
                "type": txn_type,
                "category": "uncategorized",
                "amount": amount,
                "currency": self.currency.clone(),
                "description": description,
                "txn_id": txn_id
            }));
        }

        Ok(out)
    }
}

/// Merges WeChat Pay transactions into an existing database.json Value.
/// Assumes database.json has a top level "transactions": [] array.
/// Automatically skips duplicate transactions based on txn_id.
pub fn merge_transactions_into_template(
    template: Value,
    new_txns: Vec<Value>,
) -> Result<(Value, utils::transactions::MergeStats)> {
    utils::merge_transactions_with_deduplication(template, new_txns)
}

/// Merges WeChat Pay account entries into an existing database.json Value.
/// Assumes database.json has a top level "accounts": [] array.
/// Automatically skips duplicate accounts based on account_id.
pub fn merge_accounts_into_template(
    template: Value,
    new_accounts: Vec<Value>,
) -> Result<(Value, utils::accounts::MergeStats)> {
    utils::merge_accounts_with_deduplication(template, new_accounts)
}

fn find_header_row(
    range: &calamine::Range<Data>,
) -> Result<(usize, std::collections::HashMap<String, usize>)> {
    let mut best: Option<(usize, std::collections::HashMap<String, usize>)> = None;

    for (r_idx, row) in range.rows().enumerate() {
        let first = cell_str(row.get(0)).trim().to_string();
        if first == "交易时间" {
            let mut map = std::collections::HashMap::new();
            for (c_idx, cell) in row.iter().enumerate() {
                let name = cell_str(Some(cell)).trim().to_string();
                if !name.is_empty() {
                    map.insert(name, c_idx);
                }
            }
            best = Some((r_idx, map));
            break;
        }
    }

    best.ok_or_else(|| anyhow!("Header row not found"))
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
        Data::DateTime(f) => f.to_string(),
        Data::Empty => String::new(),
        _ => c.to_string(),
    }
}

fn parse_wechat_datetime(raw: &str) -> Result<NaiveDate> {
    // Typical: 2025-03-30 01:07:53
    let dt = NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")?;
    Ok(dt.date())
}

fn parse_wechat_amount(raw: &str) -> Result<f64> {
    // Typical: ¥500.00 or -¥12.00 sometimes
    let s = raw.trim();
    if s.is_empty() {
        return Err(anyhow!("empty amount"));
    }
    let s = s.replace("¥", "").replace(",", "").trim().to_string();
    Ok(s.parse::<f64>()?)
}

fn classify_txn_type(inout: &str, wx_type: &str, amount: f64) -> String {
    // Most rows: 收/支 is 支出 or 收入
    if inout == "支出" {
        return "expense".to_string();
    }
    if inout == "收入" {
        return "income".to_string();
    }

    // When 收/支 is "/" for topups etc:
    if wx_type.contains("充值") {
        return "income".to_string();
    }
    if wx_type.contains("提现") {
        return "expense".to_string();
    }
    if wx_type.contains("退款") {
        return "income".to_string();
    }

    // Fallback to sign
    if amount < 0.0 {
        "expense".to_string()
    } else {
        "income".to_string()
    }
}

fn make_wechat_txn_id(
    account_id: &str,
    datetime_raw: &str,
    amount: f64,
    currency: &str,
    description: &str,
    trade_no: &str,
    row_index: usize,
) -> String {
    let s = format!(
        "{}|{}|{:.8}|{}|{}|{}|{}",
        account_id,
        datetime_raw.trim(),
        amount,
        currency,
        description.trim(),
        trade_no.trim(),
        row_index
    );

    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let hash = hasher.finalize();

    format!("WECHAT-{}", hex::encode(&hash[..12]))
}
