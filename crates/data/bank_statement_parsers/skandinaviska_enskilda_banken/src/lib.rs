use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook, Data, Reader, Xlsx};
use chrono::NaiveDate;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const PARSER_NAME: &str = "skandinaviska_enskilda_banken";

pub struct SebXlsxParser {
    pub account_id_checking: String,
    pub account_id_savings: String,
}

impl SebXlsxParser {
    pub fn new(checking_account_id: impl Into<String>, savings_account_id: impl Into<String>) -> Self {
        Self {
            account_id_checking: checking_account_id.into(),
            account_id_savings: savings_account_id.into(),
        }
    }

    /// Creates account entries for the SEB accounts used by this parser.
    /// Returns a vector with the checking account and savings account.
    /// Note: Some fields (like IBAN, BIC, account_number, owner, etc.)
    /// cannot be determined from Excel files and are left as null for manual completion.
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
            })
        ]
    }

    /// Parse a single Excel file and return transactions
    /// account_id: which account these transactions belong to
    pub fn parse_file<P: AsRef<Path>>(&self, path: P, account_id: &str) -> Result<Vec<Value>> {
        let mut workbook: Xlsx<_> = open_workbook(path.as_ref())
            .with_context(|| format!("Failed to open Excel file: {}", path.as_ref().display()))?;

        // Get the first worksheet
        let sheet_name = workbook
            .sheet_names()
            .first()
            .ok_or_else(|| anyhow!("No sheets found in Excel file"))?
            .clone();

        let range = workbook
            .worksheet_range(&sheet_name)
            .context("Failed to read worksheet range")?;

        let mut transactions = Vec::new();

        // Assuming Excel format has headers in first row
        // Expected columns: Date, Description, Amount, Balance (or similar)
        // We'll need to detect the actual format from the first few rows
        
        let rows: Vec<_> = range.rows().collect();
        if rows.is_empty() {
            return Ok(transactions);
        }

        // Try to find header row and column indices
        let (header_row_idx, col_date, col_description, col_amount, _col_balance, col_currency) = 
            find_columns(&rows)?;

        // Process data rows (skip header)
        for (idx, row) in rows.iter().enumerate().skip(header_row_idx + 1) {
            // Skip empty rows
            if row.is_empty() || row.iter().all(|cell| matches!(cell, Data::Empty)) {
                continue;
            }

            // Extract fields
            let date = parse_date_cell(&row, col_date)
                .with_context(|| format!("Failed to parse date at row {}", idx + 1))?;

            let description = get_string_cell(&row, col_description)
                .unwrap_or_else(|| "".to_string());

            let amount = parse_amount_cell(&row, col_amount)
                .with_context(|| format!("Failed to parse amount at row {}", idx + 1))?;

            // Currency might be in a separate column or part of the amount cell
            let currency = if let Some(curr_col) = col_currency {
                get_string_cell(&row, curr_col).unwrap_or_else(|| "SEK".to_string())
            } else {
                "SEK".to_string()
            };

            // Determine transaction type and accounts
            let txn_type = infer_type(amount, &description);
            let (from_account_id, to_account_id) = determine_accounts(
                account_id,
                &txn_type,
                amount,
                &description,
            );

            let txn_id = make_txn_id(
                account_id,
                date,
                amount,
                &currency,
                &description,
                idx + 1,
            );

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

            transactions.push(txn);
        }

        Ok(transactions)
    }
}

/// Try to find column indices for expected fields
/// Returns: (header_row_idx, date_col, description_col, amount_col, balance_col, currency_col)
fn find_columns(rows: &[&[Data]]) -> Result<(usize, usize, usize, usize, Option<usize>, Option<usize>)> {
    // Try to find header row (usually first few rows)
    for (row_idx, row) in rows.iter().enumerate().take(20) {
        let headers: Vec<String> = row.iter()
            .map(|cell| cell.to_string().to_lowercase())
            .collect();

        // Look for date column - SEB specific patterns
        let date_col = headers.iter().position(|h| 
            h.contains("booking date") || h.contains("bokföringsdatum") ||
            h.contains("date") || h.contains("datum")
        );

        // Look for description column - SEB specific patterns
        let desc_col = headers.iter().position(|h| 
            h.contains("text") || h.contains("description") || 
            h.contains("beskrivning") || h.contains("transaktion")
        );

        // Look for amount column - SEB specific patterns
        let amount_col = headers.iter().position(|h| 
            (h.contains("amount") || h.contains("belopp") || h.contains("summa")) &&
            !h.contains("balance") && !h.contains("saldo")
        );

        // Look for balance column (optional)
        let balance_col = headers.iter().position(|h| 
            h.contains("balance") || h.contains("saldo")
        );

        // Look for currency column (optional)
        let currency_col = headers.iter().position(|h| 
            h.contains("currency") || h.contains("valuta")
        );

        if let (Some(date), Some(desc), Some(amt)) = (date_col, desc_col, amount_col) {
            return Ok((row_idx, date, desc, amt, balance_col, currency_col));
        }
    }

    // If no header found, assume standard layout: Date(0), Description(1), Amount(2), Balance(3)
    if rows.len() > 1 {
        // Verify this looks like data by checking if we can parse amounts
        let test_row = rows[1];
        if test_row.len() >= 3 {
            return Ok((0, 0, 1, 2, Some(3), None));
        }
    }

    Err(anyhow!("Could not determine column layout from Excel file"))
}

fn parse_date_cell(row: &[Data], col: usize) -> Result<NaiveDate> {
    if col >= row.len() {
        return Err(anyhow!("Date column index out of bounds"));
    }

    match &row[col] {
        Data::DateTime(dt) => {
            // calamine DateTime is days since 1899-12-30
            let base = NaiveDate::from_ymd_opt(1899, 12, 30)
                .ok_or_else(|| anyhow!("Invalid base date"))?;
            let days = dt.as_f64().floor() as i64;
            base.checked_add_signed(chrono::Duration::days(days))
                .ok_or_else(|| anyhow!("Date calculation overflow"))
        }
        Data::DateTimeIso(s) | Data::DurationIso(s) => parse_date_string(s),
        Data::String(s) => parse_date_string(s),
        Data::Float(f) => {
            // Sometimes dates are stored as float (Excel serial date)
            let base = NaiveDate::from_ymd_opt(1899, 12, 30)
                .ok_or_else(|| anyhow!("Invalid base date"))?;
            let days = f.floor() as i64;
            base.checked_add_signed(chrono::Duration::days(days))
                .ok_or_else(|| anyhow!("Date calculation overflow"))
        }
        Data::Int(i) => {
            let base = NaiveDate::from_ymd_opt(1899, 12, 30)
                .ok_or_else(|| anyhow!("Invalid base date"))?;
            base.checked_add_signed(chrono::Duration::days(*i))
                .ok_or_else(|| anyhow!("Date calculation overflow"))
        }
        _ => Err(anyhow!("Unsupported date cell type: {:?}", row[col])),
    }
}

fn parse_date_string(s: &str) -> Result<NaiveDate> {
    // Try common date formats
    let formats = vec![
        "%Y-%m-%d",
        "%d/%m/%Y",
        "%d.%m.%Y",
        "%Y/%m/%d",
        "%d-%m-%Y",
        "%m/%d/%Y",
    ];

    for fmt in formats {
        if let Ok(date) = NaiveDate::parse_from_str(s, fmt) {
            return Ok(date);
        }
    }

    Err(anyhow!("Could not parse date string: {}", s))
}

fn get_string_cell(row: &[Data], col: usize) -> Option<String> {
    if col >= row.len() {
        return None;
    }

    match &row[col] {
        Data::String(s) => Some(s.clone()),
        Data::Float(f) => Some(f.to_string()),
        Data::Int(i) => Some(i.to_string()),
        Data::Bool(b) => Some(b.to_string()),
        Data::Empty => None,
        _ => Some(row[col].to_string()),
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
            // Try to parse string as number, removing common formatting
            let cleaned = s
                .replace(" ", "")
                .replace("\u{A0}", "") // non-breaking space
                .replace(",", ".");
            
            cleaned.parse::<f64>()
                .with_context(|| format!("Failed to parse amount: {}", s))
        }
        _ => Err(anyhow!("Unsupported amount cell type: {:?}", row[col])),
    }
}

fn infer_type(amount: f64, description: &str) -> String {
    let desc_lower = description.to_lowercase();

    // Check for internal transfers
    if desc_lower.contains("överföring") || desc_lower.contains("transfer") {
        return "internal_transfer".to_string();
    }

    // Check for specific income patterns
    if desc_lower.contains("lön") || desc_lower.contains("salary") || 
       desc_lower.contains("income") || desc_lower.contains("inkomst") {
        return "income".to_string();
    }

    // Default based on amount sign
    if amount < 0.0 {
        "expense".to_string()
    } else {
        "income".to_string()
    }
}

fn determine_accounts(
    account_id: &str,
    txn_type: &str,
    amount: f64,
    _description: &str,
) -> (String, String) {
    match txn_type {
        "internal_transfer" => {
            // For internal transfers, we'd need more context
            // For now, treat as transfer to/from unknown internal account
            if amount < 0.0 {
                (account_id.to_string(), "INTERNAL_DESTINATION".to_string())
            } else {
                ("INTERNAL_SOURCE".to_string(), account_id.to_string())
            }
        }
        "expense" => {
            // Money leaving the account
            (account_id.to_string(), "EXTERNAL_PAYEE".to_string())
        }
        "income" => {
            // Money coming into the account
            ("EXTERNAL_PAYER".to_string(), account_id.to_string())
        }
        _ => {
            // Default case
            if amount < 0.0 {
                (account_id.to_string(), "EXTERNAL_PAYEE".to_string())
            } else {
                ("EXTERNAL_PAYER".to_string(), account_id.to_string())
            }
        }
    }
}

fn make_txn_id(
    account_id: &str,
    date: NaiveDate,
    amount: f64,
    currency: &str,
    description: &str,
    row_index: usize,
) -> String {
    let s = format!(
        "{}|{}|{:.8}|{}|{}|{}",
        account_id,
        date.format("%Y-%m-%d"),
        amount,
        currency,
        description.trim(),
        row_index
    );

    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let hash = hasher.finalize();

    format!("SEB-{}", hex::encode(&hash[..12]))
}

/// Merges SEB transactions into an existing database.json Value.
/// Assumes database.json has a top level "transactions": [] array.
/// Automatically skips duplicate transactions based on txn_id.
///
/// # Returns
/// * `Result<(Value, utils::transactions::MergeStats)>` - The merged database and merge statistics
pub fn merge_transactions_into_template(
    template: Value,
    new_txns: Vec<Value>,
) -> Result<(Value, utils::transactions::MergeStats)> {
    utils::merge_transactions_with_deduplication(template, new_txns)
}

/// Merges SEB account entries into an existing database.json Value.
/// Assumes database.json has a top level "accounts": [] array.
/// Automatically skips duplicate accounts based on account_id.
///
/// # Returns
/// * `Result<(Value, utils::accounts::MergeStats)>` - The merged database and merge statistics
pub fn merge_accounts_into_template(
    template: Value,
    new_accounts: Vec<Value>,
) -> Result<(Value, utils::accounts::MergeStats)> {
    utils::merge_accounts_with_deduplication(template, new_accounts)
}
