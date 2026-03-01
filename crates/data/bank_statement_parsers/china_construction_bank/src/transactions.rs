use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook, Data, Reader, Xls};
use chrono::NaiveDate;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use utils::{build_transaction, TransactionInput};

use crate::CcbXlsParser;

pub fn parse_transactions(parser: &CcbXlsParser, xls_path: &str) -> Result<Vec<Value>> {
    let mut workbook: Xls<_> =
        open_workbook(xls_path).with_context(|| format!("Cannot open {}", xls_path))?;

    let sheet_name = workbook
        .sheet_names()
        .first()
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
        let seq = cell_str(row.get(c_seq)).trim().to_string();
        let summary = cell_str(row.get(c_summary)).trim().to_string();
        let date_raw = cell_str(row.get(c_date)).trim().to_string();
        let amount_raw = cell_str(row.get(c_amount)).trim().to_string();

        let is_empty_row = seq.is_empty()
            && summary.is_empty()
            && date_raw.is_empty()
            && amount_raw.is_empty();
        if is_empty_row {
            if parser.only_nonempty_rows {
                continue;
            }
            break;
        }

        if date_raw.is_empty() || amount_raw.is_empty() {
            continue;
        }

        let date = parse_ccb_date(&date_raw)
            .with_context(|| format!("Invalid date '{}' in {}", date_raw, xls_path))?;

        let amount_signed = parse_ccb_amount(&amount_raw)
            .with_context(|| format!("Invalid amount '{}' in {}", amount_raw, xls_path))?;

        let base_type = infer_base_type(amount_signed);
        let amount = amount_signed.abs();

        let currency_raw = cell_str(row.get(c_currency)).trim().to_string();
        let currency = field_currency(&currency_raw);

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

        let description = field_description(&summary, &location, &counterparty);

        out.push(build_transaction(&TransactionInput {
            date: field_date(date),
            from_account_id: field_from_account_id(parser, base_type),
            to_account_id: field_to_account_id(parser, base_type),
            transaction_type: field_type(base_type),
            category: field_category(),
            amount: field_amount(amount),
            currency,
            description: description.clone(),
            description_en: field_description_en(),
            txn_id: field_txn_id(parser, date, amount, &description, &seq, row_idx + 1),
        }));
    }

    Ok(out)
}

fn field_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn field_from_account_id(parser: &CcbXlsParser, base_type: &str) -> String {
    if base_type == "expense" {
        parser.account_id.clone()
    } else {
        "EXTERNAL_PAYER".to_string()
    }
}

fn field_to_account_id(parser: &CcbXlsParser, base_type: &str) -> String {
    if base_type == "expense" {
        "EXTERNAL_PAYEE".to_string()
    } else {
        parser.account_id.clone()
    }
}

fn field_type(base_type: &str) -> String {
    base_type.to_string()
}

fn field_category() -> String {
    "uncategorized".to_string()
}

fn field_amount(amount: f64) -> f64 {
    amount
}

fn field_currency(currency_raw: &str) -> String {
    normalize_ccb_currency(currency_raw).to_string()
}

fn field_description(summary: &str, location: &str, counterparty: &str) -> String {
    let mut desc_parts: Vec<String> = Vec::new();

    if !summary.is_empty() {
        desc_parts.push(summary.to_string());
    }
    if !location.is_empty() && location != "***" {
        desc_parts.push(location.to_string());
    }
    if !counterparty.is_empty() {
        desc_parts.push(counterparty.to_string());
    }

    if desc_parts.is_empty() {
        "CCB transaction".to_string()
    } else {
        desc_parts.join(" ")
    }
}

fn field_description_en() -> Option<String> {
    None
}

fn field_txn_id(
    parser: &CcbXlsParser,
    date: NaiveDate,
    amount: f64,
    description: &str,
    seq: &str,
    row_index: usize,
) -> String {
    make_ccb_txn_id(
        &parser.account_id,
        date,
        amount,
        "CNY",
        description,
        seq,
        row_index,
    )
}

fn infer_base_type(amount_signed: f64) -> &'static str {
    if amount_signed < 0.0 {
        "expense"
    } else {
        "income"
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

    if s.len() == 8 && s.chars().all(|c| c.is_ascii_digit()) {
        let y: i32 = s[0..4].parse()?;
        let m: u32 = s[4..6].parse()?;
        let d: u32 = s[6..8].parse()?;
        return NaiveDate::from_ymd_opt(y, m, d).ok_or_else(|| anyhow!("invalid ymd"));
    }

    if let Ok(dt) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(dt);
    }

    Err(anyhow!("unsupported date format: {}", s))
}

fn parse_ccb_amount(raw: &str) -> Result<f64> {
    let mut s = raw.trim().to_string();
    s = s.replace(',', "");
    s = s.trim().to_string();

    if s.is_empty() {
        return Err(anyhow!("empty amount"));
    }
    Ok(s.parse::<f64>()?)
}

fn normalize_ccb_currency(currency_raw: &str) -> &'static str {
    if currency_raw.contains("人民币") {
        "CNY"
    } else {
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
    let seed = format!(
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
    hasher.update(seed.as_bytes());
    let hash = hasher.finalize();

    format!("CCB-{}", hex::encode(&hash[..12]))
}
