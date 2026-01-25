use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook, Data, Reader, Xlsx};
use chrono::{Duration, NaiveDate, NaiveDateTime};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const PARSER_NAME: &str = "carpay";

pub struct CarPayXlsxParser {
    pub account_id: String,
    pub currency: String,
}

impl CarPayXlsxParser {
    pub fn new(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            currency: "SEK".to_string(),
        }
    }

    pub fn with_currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = currency.into();
        self
    }

    pub fn create_account(&self) -> Value {
        json!({
            "account_id": self.account_id,
            "structural_type": "credit_card",
            "institution": "Volvo CarPay",
            "country": "SE",
            "iban": null,
            "bic": null,
            "account_number": null,
            "owner": "self",
            "is_liability": true,
            "supports_positions": false,
            "opened_date": null,
            "closed_date": null,
            "is_active": true,
            "notes": "CarPay/Volvokort export from Kostnadsuppföljning - parsed from all sheets with transaction rows"
        })
    }

    /// Parse a whole workbook file (all sheets that contain a transaction table).
    pub fn parse_file(&self, xlsx_path: &str) -> Result<Vec<Value>> {
        let mut workbook: Xlsx<_> =
            open_workbook(xlsx_path).with_context(|| format!("Cannot open {}", xlsx_path))?;

        let sheet_names = workbook.sheet_names().to_vec();
        let mut out = Vec::new();

        for sheet_name in sheet_names {
            let range = match workbook.worksheet_range(&sheet_name) {
                Ok(r) => r,
                Err(_) => continue,
            };

            // Only parse sheets that look like transaction tables.
            // We require: Kontonummer + Datum + Belopp
            let header = find_header_row(&range);
            let Some((header_row_idx, header_map)) = header else {
                continue;
            };

            let c_date = *header_map.get("Datum").context("Missing column: Datum")?;
            let c_amount = *header_map.get("Belopp").context("Missing column: Belopp")?;

            let c_ref = header_map.get("Referens").copied();
            let c_merchant = header_map.get("Försäljningsställe").copied();
            let c_varuslag = header_map.get("Varuslag").copied();
            let c_card = header_map.get("Kort").copied();
            let c_cardtext = header_map.get("Korttext").copied();

            for (row_idx, row) in range.rows().enumerate().skip(header_row_idx + 1) {
                let date_cell = row.get(c_date);
                let amount_cell = row.get(c_amount);

                let date = match parse_excel_date(date_cell) {
                    Ok(d) => d,
                    Err(_) => {
                        // Many sheets end with empty rows after data.
                        // If both date and amount are empty, skip quietly.
                        let ds = cell_str(date_cell).unwrap_or_default();
                        let as_ = cell_str(amount_cell).unwrap_or_default();
                        if ds.trim().is_empty() && as_.trim().is_empty() {
                            continue;
                        }
                        // Otherwise, treat as a real error (format drift)
                        return Err(anyhow!(
                            "Invalid date at sheet '{}' row {} in {}",
                            sheet_name,
                            row_idx + 1,
                            xlsx_path
                        ));
                    }
                };

                let amount = match parse_amount(amount_cell) {
                    Ok(a) => a,
                    Err(_) => {
                        let ds = cell_str(date_cell).unwrap_or_default();
                        let as_ = cell_str(amount_cell).unwrap_or_default();
                        if ds.trim().is_empty() && as_.trim().is_empty() {
                            continue;
                        }
                        return Err(anyhow!(
                            "Invalid amount at sheet '{}' row {} in {}",
                            sheet_name,
                            row_idx + 1,
                            xlsx_path
                        ));
                    }
                };

                // If amount is zero, it is usually noise.
                if amount.abs() < 1e-9 {
                    continue;
                }

                let txn_type = if amount > 0.0 { "expense" } else { "income" };

                let reference = c_ref
                    .and_then(|idx| cell_str(row.get(idx)))
                    .unwrap_or_default()
                    .trim()
                    .to_string();

                let merchant = c_merchant
                    .and_then(|idx| cell_str(row.get(idx)))
                    .unwrap_or_default()
                    .trim()
                    .to_string();

                let varuslag = c_varuslag
                    .and_then(|idx| cell_str(row.get(idx)))
                    .unwrap_or_default()
                    .trim()
                    .to_string();

                let card = c_card
                    .and_then(|idx| cell_str(row.get(idx)))
                    .unwrap_or_default()
                    .trim()
                    .to_string();

                let cardtext = c_cardtext
                    .and_then(|idx| cell_str(row.get(idx)))
                    .unwrap_or_default()
                    .trim()
                    .to_string();

                // Build description with sheet context so you can later map categories easily.
                // Example: "BOLT.EU ... Visa [Övrigt] ref=... card=**** 8566"
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

                let description = if parts.is_empty() {
                    format!("CarPay transaction [{}]", sheet_name)
                } else {
                    parts.join(" ")
                };

                let (from_account_id, to_account_id) = if txn_type == "expense" {
                    (self.account_id.clone(), "EXTERNAL_PAYEE".to_string())
                } else {
                    ("EXTERNAL_PAYER".to_string(), self.account_id.clone())
                };

                let txn_id = make_txn_id(
                    &self.account_id,
                    date,
                    amount,
                    &self.currency,
                    &description,
                    &sheet_name,
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
                    "description": description,
                    "txn_id": txn_id
                }));
            }
        }

        Ok(out)
    }
}

/// Find header row for transaction sheets.
/// We require at least: Kontonummer, Datum, Belopp.
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

/// Handles dates that may come as:
/// - Excel serial number
/// - actual datetime string: "2025-08-25 00:00:00"
/// - date string: "2025-08-25"
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

/// Excel serial date conversion using 1899-12-30 base (common convention).
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
            let s = s.trim().replace(",", "");
            if s.is_empty() {
                return Err(anyhow!("empty amount string"));
            }
            Ok(s.parse::<f64>()?)
        }
        _ => {
            let s = c.to_string().trim().replace(",", "");
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
    let s = format!(
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
    hasher.update(s.as_bytes());
    let hash = hasher.finalize();

    format!("CARPAY-{}", hex::encode(&hash[..12]))
}
