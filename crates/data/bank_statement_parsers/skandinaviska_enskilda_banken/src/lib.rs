use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook, Data, Reader, Xlsx};
use chrono::NaiveDate;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const PARSER_NAME: &str = "skandinaviska_enskilda_banken";

#[derive(Clone, Debug)]
struct SebMeta {
    // Normalized digits-only, e.g. "50200105205"
    account_number_digits: Option<String>,
}

pub struct SebXlsxParser {
    pub account_id_checking: String,
    pub account_id_savings: String,

    // Optional, but helps internal transfer mapping
    pub checking_account_number_digits: Option<String>,
    pub savings_account_number_digits: Option<String>,
}

impl SebXlsxParser {
    pub fn new(checking_account_id: impl Into<String>, savings_account_id: impl Into<String>) -> Self {
        Self {
            account_id_checking: checking_account_id.into(),
            account_id_savings: savings_account_id.into(),
            checking_account_number_digits: None,
            savings_account_number_digits: None,
        }
    }

    /// If you know the account numbers (digits-only) you can set them and improve internal transfer detection.
    /// Example digits-only: "50200105205", "50371807786"
    pub fn with_account_numbers(
        mut self,
        checking_digits: Option<String>,
        savings_digits: Option<String>,
    ) -> Self {
        self.checking_account_number_digits = checking_digits;
        self.savings_account_number_digits = savings_digits;
        self
    }

    pub fn create_accounts(&self) -> Vec<Value> {
        vec![
            json!({
                "account_id": self.account_id_checking,
                "structural_type": "bank",
                "institution": "Skandinaviska Enskilda Banken",
                "country": "SE",
                "iban": null,
                "bic": null,
                "account_number": null,
                "owner": "self",
                "is_liability": false,
                "supports_positions": false,
                "opened_date": null,
                "closed_date": null,
                "is_active": true,
                "notes": "SEB checking account - some fields need manual completion"
            }),
            json!({
                "account_id": self.account_id_savings,
                "structural_type": "bank",
                "institution": "Skandinaviska Enskilda Banken",
                "country": "SE",
                "iban": null,
                "bic": null,
                "account_number": null,
                "owner": "self",
                "is_liability": false,
                "supports_positions": false,
                "opened_date": null,
                "closed_date": null,
                "is_active": true,
                "notes": "SEB savings account - some fields need manual completion"
            }),
        ]
    }

    /// Parse a single Excel file and return transactions.
    /// Pass `account_id` as either checking or savings id.
    pub fn parse_file<P: AsRef<Path>>(&self, path: P, account_id: &str) -> Result<Vec<Value>> {
        let mut workbook: Xlsx<_> = open_workbook(path.as_ref())
            .with_context(|| format!("Failed to open Excel file: {}", path.as_ref().display()))?;

        let sheet_name = workbook
            .sheet_names()
            .first()
            .ok_or_else(|| anyhow!("No sheets found in Excel file"))?
            .clone();

        let range = workbook
            .worksheet_range(&sheet_name)
            .context("Failed to read worksheet range")?;

        let rows: Vec<&[Data]> = range.rows().collect();
        if rows.is_empty() {
            return Ok(vec![]);
        }

        // Grab account number from the preamble (helps transfer mapping)
        let file_meta = extract_meta(&rows);

        // Find header and columns
        let (header_row_idx, col_date, col_description, col_amount, col_currency) = find_columns(&rows)?;

        let mut out = Vec::new();

        for (idx, row) in rows.iter().enumerate().skip(header_row_idx + 1) {
            if row.is_empty() || row.iter().all(|c| matches!(c, Data::Empty)) {
                continue;
            }

            let date = parse_date_cell(row, col_date)
                .with_context(|| format!("Failed to parse date at row {}", idx + 1))?;

            let description = get_string_cell(row, col_description).unwrap_or_default();

            let amount = parse_amount_cell(row, col_amount)
                .with_context(|| format!("Failed to parse amount at row {}", idx + 1))?;

            let currency = if let Some(c) = col_currency {
                get_string_cell(row, c).unwrap_or_else(|| "SEK".to_string())
            } else {
                "SEK".to_string()
            };

            let txn_type = infer_type(amount, &description);

            let (from_account_id, to_account_id) = determine_accounts(
                account_id,
                &txn_type,
                amount,
                &description,
                &file_meta,
                self,
            );

            let txn_id = make_txn_id(account_id, date, amount, &currency, &description, idx + 1);

            // Important: keep amount positive in the final JSON, direction is encoded by from/to.
            let txn = json!({
                "date": date.format("%Y-%m-%d").to_string(),
                "from_account_id": from_account_id,
                "to_account_id": to_account_id,
                "type": txn_type,
                "category": "uncategorized",
                "amount": amount.abs(),
                "currency": currency,
                "description": description,
                "txn_id": txn_id
            });

            out.push(txn);
        }

        Ok(out)
    }
}

/// Returns (header_row_idx, date_col, description_col, amount_col, currency_col)
fn find_columns(rows: &[&[Data]]) -> Result<(usize, usize, usize, usize, Option<usize>)> {
    for (row_idx, row) in rows.iter().enumerate().take(30) {
        let headers: Vec<String> = row.iter().map(|c| c.to_string().to_lowercase()).collect();

        let date_col = headers.iter().position(|h| {
            h.contains("booking date") || h.contains("bokföringsdatum") || h == "date" || h == "datum"
        });

        let desc_col = headers.iter().position(|h| {
            h == "text" || h.contains("description") || h.contains("beskrivning") || h.contains("transaktion")
        });

        let amount_col = headers.iter().position(|h| {
            (h == "amount" || h.contains("belopp") || h.contains("summa"))
                && !h.contains("balance")
                && !h.contains("saldo")
        });

        let currency_col = headers
            .iter()
            .position(|h| h.contains("currency") || h.contains("valuta"));

        if let (Some(d), Some(t), Some(a)) = (date_col, desc_col, amount_col) {
            return Ok((row_idx, d, t, a, currency_col));
        }
    }

    Err(anyhow!("Could not determine column layout from Excel file"))
}

fn extract_meta(rows: &[&[Data]]) -> SebMeta {
    // SEB line example:
    // "Privatkonto (5020 01 052 05)"
    // "Enkla sparkontot (5037 18 077 86)"
    for row in rows.iter().take(20) {
        for cell in row.iter() {
            let s = cell.to_string();
            if s.contains('(') && s.contains(')') {
                if let Some(inner) = s.split('(').nth(1).and_then(|x| x.split(')').next()) {
                    let digits: String = inner.chars().filter(|c| c.is_ascii_digit()).collect();
                    if digits.len() >= 8 {
                        return SebMeta {
                            account_number_digits: Some(digits),
                        };
                    }
                }
            }
        }
    }

    SebMeta {
        account_number_digits: None,
    }
}

fn parse_date_cell(row: &[Data], col: usize) -> Result<NaiveDate> {
    if col >= row.len() {
        return Err(anyhow!("Date column index out of bounds"));
    }

    match &row[col] {
        Data::String(s) => parse_date_string(s),
        Data::DateTime(dt) => {
            // Excel serial date
            let base = NaiveDate::from_ymd_opt(1899, 12, 30).ok_or_else(|| anyhow!("Invalid base date"))?;
            let days = dt.as_f64().floor() as i64;
            base.checked_add_signed(chrono::Duration::days(days))
                .ok_or_else(|| anyhow!("Date calculation overflow"))
        }
        Data::Float(f) => {
            let base = NaiveDate::from_ymd_opt(1899, 12, 30).ok_or_else(|| anyhow!("Invalid base date"))?;
            let days = f.floor() as i64;
            base.checked_add_signed(chrono::Duration::days(days))
                .ok_or_else(|| anyhow!("Date calculation overflow"))
        }
        Data::Int(i) => {
            let base = NaiveDate::from_ymd_opt(1899, 12, 30).ok_or_else(|| anyhow!("Invalid base date"))?;
            base.checked_add_signed(chrono::Duration::days(*i))
                .ok_or_else(|| anyhow!("Date calculation overflow"))
        }
        Data::DateTimeIso(s) | Data::DurationIso(s) => parse_date_string(s),
        _ => Err(anyhow!("Unsupported date cell type: {:?}", row[col])),
    }
}

fn parse_date_string(s: &str) -> Result<NaiveDate> {
    let formats = [
        "%Y-%m-%d",
        "%Y-%m-%d %H:%M:%S",
        "%d/%m/%Y",
        "%d.%m.%Y",
        "%Y/%m/%d",
        "%d-%m-%Y",
        "%m/%d/%Y",
    ];

    for fmt in formats {
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            return Ok(d);
        }
    }

    Err(anyhow!("Could not parse date string: {}", s))
}

fn get_string_cell(row: &[Data], col: usize) -> Option<String> {
    if col >= row.len() {
        return None;
    }

    match &row[col] {
        Data::String(s) => Some(s.trim().to_string()),
        Data::Float(f) => Some(f.to_string()),
        Data::Int(i) => Some(i.to_string()),
        Data::Bool(b) => Some(b.to_string()),
        Data::Empty => None,
        other => Some(other.to_string()),
    }
}

fn parse_amount_cell(row: &[Data], col: usize) -> Result<f64> {
    if col >= row.len() {
        return Err(anyhow!("Amount column index out of bounds"));
    }

    match &row[col] {
        Data::Float(f) => Ok(*f),
        Data::Int(i) => Ok(*i as f64),
        Data::String(s) => {
            let cleaned = s
                .replace(' ', "")
                .replace('\u{A0}', "")
                .replace(',', ".");
            cleaned.parse::<f64>().with_context(|| format!("Failed to parse amount: {}", s))
        }
        _ => Err(anyhow!("Unsupported amount cell type: {:?}", row[col])),
    }
}

fn infer_type(amount: f64, description: &str) -> String {
    let d = description.to_lowercase();

    // SEB internal transfer words
    if d.contains("överför") || d.contains("overfor") || d.contains("transfer") {
        return "internal_transfer".to_string();
    }

    if d.contains("lön") || d.contains("salary") || d.contains("income") || d.contains("inkomst") {
        return "income".to_string();
    }

    if amount < 0.0 {
        "expense".to_string()
    } else {
        "income".to_string()
    }
}

fn normalize_digits(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn determine_accounts(
    this_account_id: &str,
    txn_type: &str,
    amount: f64,
    description: &str,
    file_meta: &SebMeta,
    parser: &SebXlsxParser,
) -> (String, String) {
    match txn_type {
        "internal_transfer" => {
            // Best-effort: infer counterpart SEB account by scanning "Text" for the other account number.
            let desc_digits = normalize_digits(description);

            // Known digits (prefer explicitly configured, else use file meta)
            let checking_digits = parser.checking_account_number_digits.clone();
            let savings_digits = parser.savings_account_number_digits.clone();
            let this_digits = file_meta.account_number_digits.clone();

            let is_checking = this_account_id == parser.account_id_checking;
            let is_savings = this_account_id == parser.account_id_savings;

            let counterpart_id = if is_checking {
                // if description contains savings account number -> savings
                if let Some(sd) = savings_digits.as_deref() {
                    if !sd.is_empty() && desc_digits.contains(sd) {
                        Some(parser.account_id_savings.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else if is_savings {
                // if description contains checking account number -> checking
                if let Some(cd) = checking_digits.as_deref() {
                    if !cd.is_empty() && desc_digits.contains(cd) {
                        Some(parser.account_id_checking.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
            .or_else(|| {
                // fallback: if we know this file's account digits, and the description includes some digits,
                // but doesn't include the "this" digits, we still can't reliably map. Keep generic.
                let _ = this_digits;
                None
            })
            .unwrap_or_else(|| "INTERNAL_UNKNOWN".to_string());

            if amount < 0.0 {
                (this_account_id.to_string(), counterpart_id)
            } else {
                (counterpart_id, this_account_id.to_string())
            }
        }
        "expense" => (this_account_id.to_string(), "EXTERNAL_PAYEE".to_string()),
        "income" => ("EXTERNAL_PAYER".to_string(), this_account_id.to_string()),
        _ => {
            if amount < 0.0 {
                (this_account_id.to_string(), "EXTERNAL_PAYEE".to_string())
            } else {
                ("EXTERNAL_PAYER".to_string(), this_account_id.to_string())
            }
        }
    }
}

fn make_txn_id(
    account_id: &str,
    date: NaiveDate,
    signed_amount: f64,
    currency: &str,
    description: &str,
    row_index: usize,
) -> String {
    let s = format!(
        "{}|{}|{:.8}|{}|{}|{}",
        account_id,
        date.format("%Y-%m-%d"),
        signed_amount,
        currency,
        description.trim(),
        row_index
    );

    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let hash = hasher.finalize();

    format!("SEB-{}", hex::encode(&hash[..12]))
}

pub fn merge_transactions_into_template(
    template: Value,
    new_txns: Vec<Value>,
) -> Result<(Value, utils::transactions::MergeStats)> {
    utils::merge_transactions_with_deduplication(template, new_txns)
}

pub fn merge_accounts_into_template(
    template: Value,
    new_accounts: Vec<Value>,
) -> Result<(Value, utils::accounts::MergeStats)> {
    utils::merge_accounts_with_deduplication(template, new_accounts)
}
