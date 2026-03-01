use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook, Data, Reader, Xlsx};
use chrono::{NaiveDate, NaiveDateTime};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use utils::{build_transaction, TransactionInput};

use crate::WeChatXlsxParser;

pub fn parse_transactions(parser: &WeChatXlsxParser, xlsx_path: &str) -> Result<Vec<Value>> {
    let mut workbook: Xlsx<_> =
        open_workbook(xlsx_path).with_context(|| format!("Cannot open {}", xlsx_path))?;

    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("No sheets found in {}", xlsx_path))?;

    let range = workbook
        .worksheet_range(&sheet_name)
        .with_context(|| format!("Cannot read sheet '{}' in {}", sheet_name, xlsx_path))?;

    let (header_row_idx, header_map) =
        find_header_row(&range).context("Could not find WeChat header row starting with '交易时间'")?;

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

    for (row_idx, row) in range.rows().enumerate().skip(header_row_idx + 1) {
        let datetime_raw = cell_str(row.get(c_time)).trim().to_string();
        if datetime_raw.is_empty() {
            continue;
        }

        let status = cell_str(row.get(c_status)).trim().to_string();
        if parser.only_successful && !is_success_status(&status) {
            let inout_preview = cell_str(row.get(c_inout)).trim().to_string();
            let amount_preview = cell_str(row.get(c_amount)).trim().to_string();
            println!(
                "⚠️  Skipping transaction with status '{}' (收/支: {}, 金额: {})",
                status, inout_preview, amount_preview
            );
            continue;
        }

        let date = parse_wechat_datetime(&datetime_raw)
            .with_context(|| format!("Invalid datetime '{}' in {}", datetime_raw, xlsx_path))?;

        let amount_raw = cell_str(row.get(c_amount));
        let amount = parse_wechat_amount(&amount_raw)
            .with_context(|| format!("Invalid amount '{}' in {}", amount_raw, xlsx_path))?;

        let inout = cell_str(row.get(c_inout)).trim().to_string();
        let wx_type = cell_str(row.get(c_type)).trim().to_string();
        let txn_type = classify_txn_type(&inout, &wx_type, amount);

        let counterparty = cell_str(row.get(c_counterparty)).trim().to_string();
        let product = cell_str(row.get(c_product)).trim().to_string();
        let paymethod = cell_str(row.get(c_paymethod)).trim().to_string();
        let note = c_note
            .map(|idx| cell_str(row.get(idx)))
            .unwrap_or_default()
            .trim()
            .to_string();

        let description = build_description(&counterparty, &product, &wx_type, &paymethod, &note);
        let trade_no = cell_str(row.get(c_trade_no)).trim().to_string();

        out.push(build_transaction(&TransactionInput {
            date: field_date(date),
            from_account_id: field_from_account_id(parser, &txn_type, amount),
            to_account_id: field_to_account_id(parser, &txn_type, amount),
            transaction_type: field_type(&txn_type),
            category: field_category(),
            amount: field_amount(amount),
            currency: field_currency(parser),
            description: description.clone(),
            description_en: field_description_en(),
            txn_id: field_txn_id(
                parser,
                &datetime_raw,
                amount,
                &description,
                &trade_no,
                row_idx + 1,
            ),
        }));
    }

    Ok(out)
}

fn field_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn field_from_account_id(parser: &WeChatXlsxParser, txn_type: &str, amount: f64) -> String {
    match txn_type {
        "expense" => parser.account_id.clone(),
        "income" => "EXTERNAL_PAYER".to_string(),
        "internal_transfer" => parser.account_id.clone(),
        _ => {
            if amount < 0.0 {
                parser.account_id.clone()
            } else {
                "EXTERNAL_PAYER".to_string()
            }
        }
    }
}

fn field_to_account_id(parser: &WeChatXlsxParser, txn_type: &str, amount: f64) -> String {
    match txn_type {
        "expense" => "EXTERNAL_PAYEE".to_string(),
        "income" => parser.account_id.clone(),
        "internal_transfer" => parser.account_id.clone(),
        _ => {
            if amount < 0.0 {
                "EXTERNAL_PAYEE".to_string()
            } else {
                parser.account_id.clone()
            }
        }
    }
}

fn field_type(txn_type: &str) -> String {
    txn_type.to_string()
}

fn field_category() -> String {
    "uncategorized".to_string()
}

fn field_amount(amount: f64) -> f64 {
    amount
}

fn field_currency(parser: &WeChatXlsxParser) -> String {
    parser.currency.clone()
}

fn field_description_en() -> Option<String> {
    None
}

fn field_txn_id(
    parser: &WeChatXlsxParser,
    datetime_raw: &str,
    amount: f64,
    description: &str,
    trade_no: &str,
    row_index: usize,
) -> String {
    make_wechat_txn_id(
        &parser.account_id,
        datetime_raw,
        amount,
        &parser.currency,
        description,
        trade_no,
        row_index,
    )
}

fn find_header_row(range: &calamine::Range<Data>) -> Result<(usize, HashMap<String, usize>)> {
    for (r_idx, row) in range.rows().enumerate() {
        let first = cell_str(row.get(0)).trim().to_string();
        if first == "交易时间" {
            let mut map = HashMap::new();
            for (c_idx, cell) in row.iter().enumerate() {
                let name = cell_str(Some(cell)).trim().to_string();
                if !name.is_empty() {
                    map.insert(name, c_idx);
                }
            }
            return Ok((r_idx, map));
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
        Data::DateTime(f) => f.to_string(),
        Data::Empty => String::new(),
        _ => c.to_string(),
    }
}

fn parse_wechat_datetime(raw: &str) -> Result<NaiveDate> {
    let dt = NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")?;
    Ok(dt.date())
}

fn parse_wechat_amount(raw: &str) -> Result<f64> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(anyhow!("empty amount"));
    }
    let s = s.replace('¥', "").replace(',', "").trim().to_string();
    Ok(s.parse::<f64>()?)
}

fn classify_txn_type(inout: &str, wx_type: &str, amount: f64) -> String {
    if inout == "支出" {
        return "expense".to_string();
    }
    if inout == "收入" {
        return "income".to_string();
    }

    if wx_type.contains("充值") {
        return "income".to_string();
    }
    if wx_type.contains("提现") {
        return "expense".to_string();
    }
    if wx_type.contains("退款") {
        return "income".to_string();
    }

    if amount < 0.0 {
        "expense".to_string()
    } else {
        "income".to_string()
    }
}

fn is_success_status(status: &str) -> bool {
    status.contains("成功")
        || status.contains("已收钱")
        || status.contains("完成")
        || status.contains("退款")
        || status.contains("已到账")
        || status.contains("已收款")
        || status.contains("已入账")
        || status.contains("已存入")
}

fn build_description(
    counterparty: &str,
    product: &str,
    wx_type: &str,
    paymethod: &str,
    note: &str,
) -> String {
    let mut desc_parts = Vec::new();

    if !counterparty.is_empty() && counterparty != "/" {
        desc_parts.push(counterparty.to_string());
    }
    if !product.is_empty() && product != "/" {
        desc_parts.push(product.to_string());
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

    if desc_parts.is_empty() {
        "WeChat Pay transaction".to_string()
    } else {
        desc_parts.join(" ")
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
    let seed = format!(
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
    hasher.update(seed.as_bytes());
    let hash = hasher.finalize();

    format!("WECHAT-{}", hex::encode(&hash[..12]))
}
