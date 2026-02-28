use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook, Data, Reader, Xlsx};
use chrono::{Duration, NaiveDate, NaiveDateTime};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use utils::{build_transaction, TransactionInput};

use crate::CarPayXlsxParser;

struct HeaderColumns {
    date: usize,
    amount: usize,
    reference: Option<usize>,
    merchant: Option<usize>,
    varuslag: Option<usize>,
    card: Option<usize>,
    cardtext: Option<usize>,
}

pub fn parse_transactions(parser: &CarPayXlsxParser, xlsx_path: &str) -> Result<Vec<Value>> {
    let mut workbook: Xlsx<_> =
        open_workbook(xlsx_path).with_context(|| format!("Cannot open {}", xlsx_path))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let mut out = Vec::new();

    for sheet_name in sheet_names {
        let range = match workbook.worksheet_range(&sheet_name) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let Some((header_row_idx, header_map)) = find_header_row(&range) else {
            continue;
        };

        let cols = HeaderColumns {
            date: *header_map.get("Datum").context("Missing column: Datum")?,
            amount: *header_map.get("Belopp").context("Missing column: Belopp")?,
            reference: header_map.get("Referens").copied(),
            merchant: header_map.get("Försäljningsställe").copied(),
            varuslag: header_map.get("Varuslag").copied(),
            card: header_map.get("Kort").copied(),
            cardtext: header_map.get("Korttext").copied(),
        };

        for (row_idx, row) in range.rows().enumerate().skip(header_row_idx + 1) {
            let row_number = row_idx + 1;
            let date_cell = row.get(cols.date);
            let amount_cell = row.get(cols.amount);

            let date = match parse_excel_date(date_cell) {
                Ok(d) => d,
                Err(_) => {
                    if is_effectively_empty_row(date_cell, amount_cell) {
                        continue;
                    }

                    return Err(anyhow!(
                        "Invalid date at sheet '{}' row {} in {}",
                        sheet_name,
                        row_number,
                        xlsx_path
                    ));
                }
            };

            let amount_raw = match parse_amount(amount_cell) {
                Ok(a) => a,
                Err(_) => {
                    if is_effectively_empty_row(date_cell, amount_cell) {
                        continue;
                    }

                    return Err(anyhow!(
                        "Invalid amount at sheet '{}' row {} in {}",
                        sheet_name,
                        row_number,
                        xlsx_path
                    ));
                }
            };

            if amount_raw.abs() < 1e-9 {
                continue;
            }

            let base_type = infer_base_type(amount_raw);
            let normalized_amount = amount_raw.abs();
            let description = field_description(row, &cols, &sheet_name);
            let txn_id = field_txn_id(
                parser,
                date,
                normalized_amount,
                &description,
                &sheet_name,
                row_number,
            );

            out.push(build_transaction(&TransactionInput {
                date: field_date(date),
                from_account_id: field_from_account_id(parser, base_type),
                to_account_id: field_to_account_id(parser, base_type),
                transaction_type: field_type(base_type),
                category: field_category(),
                amount: field_amount(normalized_amount),
                currency: field_currency(parser),
                description,
                description_en: field_description_en(),
                txn_id,
            }));
        }
    }

    Ok(out)
}

fn infer_base_type(amount_raw: f64) -> &'static str {
    if amount_raw > 0.0 {
        "expense"
    } else {
        "income"
    }
}

fn field_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn field_from_account_id(parser: &CarPayXlsxParser, base_type: &str) -> String {
    if base_type == "expense" {
        parser.account_id.clone()
    } else {
        "EXTERNAL_PAYER".to_string()
    }
}

fn field_to_account_id(parser: &CarPayXlsxParser, base_type: &str) -> String {
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

fn field_currency(parser: &CarPayXlsxParser) -> String {
    parser.currency.clone()
}

fn field_description(row: &[Data], cols: &HeaderColumns, sheet_name: &str) -> String {
    let reference = cols
        .reference
        .and_then(|idx| cell_str(row.get(idx)))
        .unwrap_or_default()
        .trim()
        .to_string();

    let merchant = cols
        .merchant
        .and_then(|idx| cell_str(row.get(idx)))
        .unwrap_or_default()
        .trim()
        .to_string();

    let varuslag = cols
        .varuslag
        .and_then(|idx| cell_str(row.get(idx)))
        .unwrap_or_default()
        .trim()
        .to_string();

    let card = cols
        .card
        .and_then(|idx| cell_str(row.get(idx)))
        .unwrap_or_default()
        .trim()
        .to_string();

    let cardtext = cols
        .cardtext
        .and_then(|idx| cell_str(row.get(idx)))
        .unwrap_or_default()
        .trim()
        .to_string();

    let mut parts: Vec<String> = Vec::new();

    if !merchant.is_empty() {
        parts.push(trim_midspaces(&merchant));
    }
    if !varuslag.is_empty() {
        parts.push(trim_midspaces(&varuslag));
    }
    if !sheet_name.is_empty() {
        parts.push(format!("[{}]", sheet_name));
    }
    if !reference.is_empty() {
        parts.push(format!("ref={}", reference.trim()));
    }
    if !card.is_empty() {
        parts.push(format!("card={}", card.trim()));
    }
    if !cardtext.is_empty() {
        parts.push(format!("holder={}", trim_midspaces(&cardtext)));
    }

    if parts.is_empty() {
        format!("CarPay transaction [{}]", sheet_name)
    } else {
        parts.join(" ")
    }
}

fn field_description_en() -> Option<String> {
    None
}

fn field_txn_id(
    parser: &CarPayXlsxParser,
    date: NaiveDate,
    amount: f64,
    description: &str,
    sheet_name: &str,
    row_index: usize,
) -> String {
    make_txn_id(
        &parser.account_id,
        date,
        amount,
        &parser.currency,
        description,
        sheet_name,
        row_index,
    )
}

fn find_header_row(range: &calamine::Range<Data>) -> Option<(usize, HashMap<String, usize>)> {
    for (r_idx, row) in range.rows().enumerate() {
        let mut map = HashMap::new();

        for (c_idx, cell) in row.iter().enumerate() {
            let name = cell_str(Some(cell)).unwrap_or_default();
            let name = name.trim().to_string();
            if !name.is_empty() {
                map.insert(name, c_idx);
            }
        }

        let has_konto = map.contains_key("Kontonummer");
        let has_datum = map.contains_key("Datum");
        let has_belopp = map.contains_key("Belopp");

        if has_konto && has_datum && has_belopp {
            return Some((r_idx, map));
        }
    }
    None
}

fn cell_str(cell: Option<&Data>) -> Option<String> {
    let c = cell?;
    match c {
        Data::String(s) => Some(s.clone()),
        Data::Float(f) => Some(f.to_string()),
        Data::Int(i) => Some(i.to_string()),
        Data::Bool(b) => Some(b.to_string()),
        Data::Empty => None,
        _ => Some(c.to_string()),
    }
}

fn is_effectively_empty_row(date_cell: Option<&Data>, amount_cell: Option<&Data>) -> bool {
    let ds = cell_str(date_cell).unwrap_or_default();
    let as_ = cell_str(amount_cell).unwrap_or_default();
    ds.trim().is_empty() && as_.trim().is_empty()
}

fn parse_excel_date(cell: Option<&Data>) -> Result<NaiveDate> {
    let Some(c) = cell else {
        return Err(anyhow!("empty date"));
    };

    match c {
        Data::Float(f) => excel_serial_to_date(*f),
        Data::Int(i) => excel_serial_to_date(*i as f64),
        Data::String(s) => parse_date_string(s),
        Data::DateTime(dt) => excel_serial_to_date(dt.as_f64()),
        Data::DateTimeIso(s) | Data::DurationIso(s) => parse_date_string(s),
        _ => {
            let s = c.to_string();
            parse_date_string(&s)
        }
    }
}

fn parse_date_string(s: &str) -> Result<NaiveDate> {
    let s = s.trim();
    if s.is_empty() {
        return Err(anyhow!("empty date string"));
    }

    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Ok(dt.date());
    }

    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(d);
    }

    Err(anyhow!("unsupported date format: {}", s))
}

fn excel_serial_to_date(v: f64) -> Result<NaiveDate> {
    if !v.is_finite() {
        return Err(anyhow!("non-finite excel date"));
    }

    let days = v.floor() as i64;
    let base = NaiveDate::from_ymd_opt(1899, 12, 30).ok_or_else(|| anyhow!("bad base date"))?;
    Ok(base + Duration::days(days))
}

fn parse_amount(cell: Option<&Data>) -> Result<f64> {
    let Some(c) = cell else {
        return Err(anyhow!("empty amount"));
    };

    match c {
        Data::Float(f) => Ok(*f),
        Data::Int(i) => Ok(*i as f64),
        Data::String(s) => {
            let s = s.trim().replace(',', "");
            if s.is_empty() {
                return Err(anyhow!("empty amount string"));
            }
            Ok(s.parse::<f64>()?)
        }
        _ => {
            let s = c.to_string().trim().replace(',', "");
            if s.is_empty() {
                return Err(anyhow!("empty amount value"));
            }
            Ok(s.parse::<f64>()?)
        }
    }
}

fn trim_midspaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn make_txn_id(
    account_id: &str,
    date: NaiveDate,
    amount: f64,
    currency: &str,
    description: &str,
    sheet: &str,
    row_index: usize,
) -> String {
    let seed = format!(
        "{}|{}|{:.8}|{}|{}|{}|{}",
        account_id,
        date.format("%Y-%m-%d"),
        amount,
        currency,
        description.trim(),
        sheet.trim(),
        row_index
    );

    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let hash = hasher.finalize();

    format!("CARPAY-{}", hex::encode(&hash[..12]))
}
