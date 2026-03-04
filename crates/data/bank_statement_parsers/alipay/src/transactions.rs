use anyhow::{anyhow, Context, Result};
use chrono::{NaiveDate, NaiveDateTime};
use encoding_rs::GB18030;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;
use utils::{build_transaction, TransactionInput};

use crate::AlipayCsvParser;

struct AlipayColumns {
    idx_time: usize,
    idx_category: usize,
    idx_counterparty: usize,
    idx_item: usize,
    idx_inout: usize,
    idx_amount: usize,
    idx_status: usize,
    idx_order: usize,
    idx_note: Option<usize>,
}

pub fn parse_transactions<R: Read>(parser: &AlipayCsvParser, mut reader: R) -> Result<Vec<Value>> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;

    let decoded = decode_text_lossy(&buf);
    let csv_text = slice_to_csv_table(&decoded)
        .context("Could not find Alipay CSV header row starting with '交易时间,'")?;

    let mut csv_reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(csv_text.as_bytes());

    let headers = csv_reader.headers().context("Missing CSV headers")?.clone();

    let cols = AlipayColumns {
        idx_time: find_col(&headers, "交易时间")?,
        idx_category: find_col(&headers, "交易分类")?,
        idx_counterparty: find_col(&headers, "交易对方")?,
        idx_item: find_col(&headers, "商品说明")?,
        idx_inout: find_col(&headers, "收/支")?,
        idx_amount: find_col(&headers, "金额")?,
        idx_status: find_col(&headers, "交易状态")?,
        idx_order: find_col(&headers, "交易订单号")?,
        idx_note: find_optional_col(&headers, "备注"),
    };

    let mut out = Vec::new();

    for (row_idx, rec) in csv_reader.records().enumerate() {
        let rec = rec.with_context(|| format!("CSV read error at row {}", row_idx + 2))?;

        let status = status_from_row(&rec, &cols);
        if should_skip_row(parser, status) {
            continue;
        }

        let date = date_from_row(&rec, &cols, row_idx + 2)?;
        let amount = amount_from_row(&rec, &cols, row_idx + 2)?;
        let inout = inout_from_row(&rec, &cols);
        let item_raw = item_from_row(&rec, &cols);
        let counterparty_raw = counterparty_from_row(&rec, &cols);
        let category_raw = category_from_row(&rec, &cols);
        let semantic_blob = semantic_blob(item_raw, counterparty_raw, category_raw);

        let (base_type, semantic_tag) =
            infer_base_type_and_semantic_tag(inout, amount, status, &semantic_blob);
        let description = field_description(
            &rec,
            &cols,
            item_raw,
            counterparty_raw,
            category_raw,
            semantic_tag,
        );
        let order_no = order_no_from_row(&rec, &cols);
        let txn_id = field_txn_id(parser, date, amount, &description, order_no, row_idx + 1);

        out.push(build_transaction(&TransactionInput {
            date: field_date(date),
            from_account_id: field_from_account_id(parser, &base_type, amount),
            to_account_id: field_to_account_id(parser, &base_type, amount),
            transaction_type: field_type(parser, &base_type, amount),
            category: field_category(),
            amount: field_amount(amount),
            currency: field_currency(parser),
            description,
            description_en: field_description_en(),
            txn_id,
        }));
    }

    Ok(out)
}

fn should_skip_row(parser: &AlipayCsvParser, status: &str) -> bool {
    parser.only_successful && !(status == "交易成功" || status == "退款成功")
}

fn status_from_row<'a>(rec: &'a csv::StringRecord, cols: &AlipayColumns) -> &'a str {
    rec.get(cols.idx_status).unwrap_or("").trim()
}

fn date_from_row(
    rec: &csv::StringRecord,
    cols: &AlipayColumns,
    row_number: usize,
) -> Result<NaiveDate> {
    let raw_dt = rec.get(cols.idx_time).unwrap_or("").trim();
    parse_alipay_datetime(raw_dt)
        .with_context(|| format!("Invalid datetime '{}' at row {}", raw_dt, row_number))
}

fn field_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn amount_from_row(
    rec: &csv::StringRecord,
    cols: &AlipayColumns,
    row_number: usize,
) -> Result<f64> {
    let amount_raw = rec.get(cols.idx_amount).unwrap_or("");
    parse_amount(amount_raw)
        .with_context(|| format!("Invalid amount '{}' at row {}", amount_raw, row_number))
}

fn field_amount(amount: f64) -> f64 {
    amount.abs()
}

fn inout_from_row<'a>(rec: &'a csv::StringRecord, cols: &AlipayColumns) -> &'a str {
    rec.get(cols.idx_inout).unwrap_or("").trim()
}

fn item_from_row<'a>(rec: &'a csv::StringRecord, cols: &AlipayColumns) -> &'a str {
    rec.get(cols.idx_item).unwrap_or("").trim()
}

fn counterparty_from_row<'a>(rec: &'a csv::StringRecord, cols: &AlipayColumns) -> &'a str {
    rec.get(cols.idx_counterparty).unwrap_or("").trim()
}

fn category_from_row<'a>(rec: &'a csv::StringRecord, cols: &AlipayColumns) -> &'a str {
    rec.get(cols.idx_category).unwrap_or("").trim()
}

fn semantic_blob(item_raw: &str, counterparty_raw: &str, category_raw: &str) -> String {
    format!("{} {} {}", item_raw, counterparty_raw, category_raw)
}

fn infer_base_type_and_semantic_tag(
    inout: &str,
    amount: f64,
    status: &str,
    semantic_blob: &str,
) -> (String, Option<&'static str>) {
    let mut txn_type = match inout {
        "支出" => "expense",
        "收入" => "income",
        "不计收支" => {
            if amount < 0.0 {
                "expense"
            } else {
                "income"
            }
        }
        _ => {
            if amount < 0.0 {
                "expense"
            } else {
                "income"
            }
        }
    };

    let mut semantic_tag: Option<&'static str> = None;

    if status == "退款成功" || semantic_blob.contains("退款") {
        txn_type = "income";
        semantic_tag = Some("退款");
    } else if semantic_blob.contains("充值") {
        txn_type = "income";
        semantic_tag = Some("充值");
    } else if semantic_blob.contains("提现") {
        txn_type = "expense";
        semantic_tag = Some("提现");
    } else if semantic_blob.contains("收钱码收款") || semantic_blob.contains("收款") {
        txn_type = "income";
        semantic_tag = Some("收款");
    }

    (txn_type.to_string(), semantic_tag)
}

fn field_description(
    rec: &csv::StringRecord,
    cols: &AlipayColumns,
    item_raw: &str,
    counterparty_raw: &str,
    category_raw: &str,
    semantic_tag: Option<&str>,
) -> String {
    let mut description = if item_raw.is_empty() {
        counterparty_raw.to_string()
    } else {
        item_raw.to_string()
    };

    description = html_escape::decode_html_entities(&description).to_string();
    description = description.replace('\u{00A0}', " ");
    description = description.split_whitespace().collect::<Vec<_>>().join(" ");

    if !category_raw.is_empty() {
        description = format!("{} [{}]", description, category_raw);
    }

    if let Some(tag) = semantic_tag {
        description = format!("{} [{}]", description, tag);
    }

    if let Some(i) = cols.idx_note {
        let note = rec.get(i).unwrap_or("").trim();
        if !note.is_empty() {
            description = format!("{} ({})", description, note);
        }
    }

    description
}

fn field_description_en() -> Option<String> {
    None
}

fn field_currency(parser: &AlipayCsvParser) -> String {
    parser.currency.clone()
}

fn field_category() -> String {
    "uncategorized".to_string()
}

fn normalized_accounts_and_type(
    parser: &AlipayCsvParser,
    txn_type: &str,
    amount: f64,
) -> (String, String, String) {
    let (from_account_id, to_account_id) = match txn_type {
        "expense" => (parser.account_id.clone(), "EXTERNAL_PAYEE".to_string()),
        "income" => ("EXTERNAL_PAYER".to_string(), parser.account_id.clone()),
        _ => {
            if amount < 0.0 {
                (parser.account_id.clone(), "EXTERNAL_PAYEE".to_string())
            } else {
                ("EXTERNAL_PAYER".to_string(), parser.account_id.clone())
            }
        }
    };

    if from_account_id == to_account_id || txn_type == "internal_transfer" {
        if amount < 0.0 {
            (
                parser.account_id.clone(),
                "EXTERNAL_PAYEE".to_string(),
                "expense".to_string(),
            )
        } else {
            (
                "EXTERNAL_PAYER".to_string(),
                parser.account_id.clone(),
                "income".to_string(),
            )
        }
    } else {
        (from_account_id, to_account_id, txn_type.to_string())
    }
}

fn field_from_account_id(parser: &AlipayCsvParser, txn_type: &str, amount: f64) -> String {
    let (from_account_id, _, _) = normalized_accounts_and_type(parser, txn_type, amount);
    from_account_id
}

fn field_to_account_id(parser: &AlipayCsvParser, txn_type: &str, amount: f64) -> String {
    let (_, to_account_id, _) = normalized_accounts_and_type(parser, txn_type, amount);
    to_account_id
}

fn field_type(parser: &AlipayCsvParser, txn_type: &str, amount: f64) -> String {
    let (_, _, final_type) = normalized_accounts_and_type(parser, txn_type, amount);
    final_type
}

fn order_no_from_row<'a>(rec: &'a csv::StringRecord, cols: &AlipayColumns) -> &'a str {
    rec.get(cols.idx_order)
        .unwrap_or("")
        .trim()
        .trim_end_matches('\t')
}

fn field_txn_id(
    parser: &AlipayCsvParser,
    date: NaiveDate,
    amount: f64,
    description: &str,
    order_no: &str,
    row_index: usize,
) -> String {
    make_alipay_txn_id(
        &parser.account_id,
        date,
        amount,
        &parser.currency,
        description,
        order_no,
        row_index,
    )
}

fn decode_text_lossy(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);

    if let Ok(s) = std::str::from_utf8(bytes) {
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
    // Alipay exports can include either minute precision or second precision.
    for fmt in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(raw, fmt) {
            return Ok(dt.date());
        }
    }

    Err(anyhow!("unsupported datetime format: {}", raw))
}

fn parse_amount(raw: &str) -> Result<f64> {
    let s = raw.trim().replace(',', "");
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
    let seed = format!(
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
    hasher.update(seed.as_bytes());
    let hash = hasher.finalize();

    format!("ALIPAY-{}", hex::encode(&hash[..12]))
}
