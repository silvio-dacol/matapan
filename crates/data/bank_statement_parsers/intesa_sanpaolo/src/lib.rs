use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook, Reader, Xlsx};
use chrono::{Datelike, Local, NaiveDate};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

mod accounts;
mod instruments;
mod positions;
mod transactions;

pub const PARSER_NAME: &str = "intesa_sanpaolo";

pub const INTESA_CHECKING: &str = "INTESA_CHECKING";
pub const INTESA_SAVINGS: &str = "INTESA_SAVINGS";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileType {
    Transactions,
    Portfolio,
}

pub struct IntesaSanpaoloParser {
    pub account_id_checking: String,
    pub account_id_trading: String,
}

impl IntesaSanpaoloParser {
    pub fn new() -> Self {
        Self {
            account_id_checking: INTESA_CHECKING.to_string(),
            account_id_trading: INTESA_SAVINGS.to_string(),
        }
    }

    pub fn create_accounts(&self) -> Vec<Value> {
        accounts::create_all_accounts(self)
    }

    pub fn detect_file_type<P: AsRef<Path>>(&self, path: P) -> Result<FileType> {
        let path_str = path.as_ref().to_string_lossy().to_lowercase();

        if path_str.contains("patrimonio")
            || path_str.contains("portfolio")
            || path_str.contains("holdings")
        {
            return Ok(FileType::Portfolio);
        }

        if path_str.contains("movimenti")
            || path_str.contains("operazioni")
            || path_str.contains("transactions")
            || path_str.contains("intesa_sanpaolo")
        {
            return Ok(FileType::Transactions);
        }

        let mut workbook: Xlsx<_> = open_workbook(&path)
            .with_context(|| format!("Failed to open workbook: {}", path.as_ref().display()))?;

        let sheet_names = workbook.sheet_names().to_vec();
        for name in &sheet_names {
            let name_lower = name.to_lowercase();
            if name_lower.contains("lista operazione") || name_lower.contains("movimenti") {
                return Ok(FileType::Transactions);
            }
            if name_lower.contains("patrimonio") || name_lower.contains("portfolio") {
                return Ok(FileType::Portfolio);
            }
        }

        if let Some(first_sheet) = sheet_names.first() {
            if let Ok(range) = workbook.worksheet_range(first_sheet) {
                for row_idx in 0..range.get_size().0.min(15) {
                    for col_idx in 0..range.get_size().1 {
                        if let Some(cell) = range.get((row_idx, col_idx)) {
                            let text = cell.to_string().to_lowercase();
                            if text == "isin" {
                                return Ok(FileType::Portfolio);
                            }
                            if text.contains("importo") && (text.contains("data") || row_idx < 3) {
                                for check_col in 0..range.get_size().1 {
                                    if let Some(check_cell) = range.get((row_idx, check_col)) {
                                        let check_text = check_cell.to_string().to_lowercase();
                                        if check_text.contains("operazione") || check_text == "data" {
                                            return Ok(FileType::Transactions);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        println!(
            "⚠️  Could not determine file type for {}, defaulting to transactions",
            path.as_ref().display()
        );
        Ok(FileType::Transactions)
    }

    pub fn parse_file<P: AsRef<Path>>(&self, path: P) -> Result<ParsedIntesa> {
        let file_type = self.detect_file_type(&path)?;
        self.parse_file_with_type(path, file_type)
    }

    pub fn parse_file_with_type<P: AsRef<Path>>(
        &self,
        path: P,
        file_type: FileType,
    ) -> Result<ParsedIntesa> {
        let fallback_statement_date = extract_file_statement_date(path.as_ref())
            .unwrap_or_else(|| Local::now().naive_local().date());

        let mut workbook: Xlsx<_> = open_workbook(&path)
            .with_context(|| format!("Failed to open workbook: {}", path.as_ref().display()))?;

        match file_type {
            FileType::Transactions => {
                let transactions = transactions::parse_transactions(self, &mut workbook)?;
                Ok(ParsedIntesa {
                    transactions,
                    positions: Vec::new(),
                    instruments: Vec::new(),
                    file_type,
                })
            }
            FileType::Portfolio => {
                let (instruments, positions) =
                    positions::parse_portfolio(self, &mut workbook, fallback_statement_date)?;
                Ok(ParsedIntesa {
                    transactions: Vec::new(),
                    positions,
                    instruments,
                    file_type,
                })
            }
        }
    }
}

impl Default for IntesaSanpaoloParser {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ParsedIntesa {
    pub transactions: Vec<Value>,
    pub positions: Vec<Value>,
    pub instruments: Vec<Value>,
    pub file_type: FileType,
}

pub(crate) fn parse_date_or_serial(s: &str) -> Result<NaiveDate> {
    let s = s.trim();

    if let Ok(serial) = s.parse::<f64>() {
        if (1.0..100000.0).contains(&serial) {
            let days = serial.floor() as i64;
            let base_date = NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
            if let Some(date) = base_date.checked_add_signed(chrono::Duration::days(days)) {
                return Ok(date);
            }
        }
    }

    parse_date(s)
}

pub(crate) fn parse_date(s: &str) -> Result<NaiveDate> {
    let s = s.trim();

    if let Ok(date) = NaiveDate::parse_from_str(s, "%d/%m/%Y") {
        return Ok(date);
    }
    if let Ok(date) = NaiveDate::parse_from_str(s, "%d-%m-%Y") {
        return Ok(date);
    }
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(date);
    }
    if let Ok(date) = NaiveDate::parse_from_str(s, "%d.%m.%Y") {
        return Ok(date);
    }

    Err(anyhow!("Unrecognized date format: {}", s))
}

pub(crate) fn parse_amount(s: &str) -> Result<f64> {
    let s = s.trim();
    if s.is_empty() || s == "-" || s == "--" {
        return Err(anyhow!("Empty amount"));
    }

    let mut cleaned = s.to_string();
    cleaned = cleaned.replace("€", "").replace("EUR", "").replace(' ', "");

    if cleaned.contains(',') {
        cleaned = cleaned.replace('.', "");
        cleaned = cleaned.replace(',', ".");
    }

    cleaned
        .parse::<f64>()
        .with_context(|| format!("Failed to parse amount: {}", s))
}

pub(crate) fn extract_portfolio_date(
    range: &calamine::Range<calamine::Data>,
) -> Option<NaiveDate> {
    if let Some(date) = extract_portfolio_date_from_column_g(range) {
        return Some(date);
    }

    let (height, width) = range.get_size();
    for row_idx in 0..height.min(10) {
        for col_idx in 0..width {
            if let Some(cell) = range.get((row_idx, col_idx)) {
                let text = cell.to_string();
                if let Ok(date) = parse_date_or_serial(&text) {
                    if (2020..=2030).contains(&date.year()) {
                        return Some(date);
                    }
                }
            }
        }
    }

    None
}

pub(crate) fn extract_portfolio_date_from_column_g(
    range: &calamine::Range<calamine::Data>,
) -> Option<NaiveDate> {
    let (height, width) = range.get_size();
    if width <= 6 {
        return None;
    }

    let mut counts: HashMap<NaiveDate, usize> = HashMap::new();
    for row_idx in 0..height {
        if let Some(cell) = range.get((row_idx, 6)) {
            if let Ok(date) = parse_date_or_serial(&cell.to_string()) {
                if (2020..=2035).contains(&date.year()) {
                    *counts.entry(date).or_insert(0) += 1;
                }
            }
        }
    }

    counts
        .into_iter()
        .max_by_key(|(date, count)| (*count, *date))
        .map(|(date, _)| date)
}

pub(crate) fn extract_row_date_from_column_g(
    range: &calamine::Range<calamine::Data>,
    row_idx: usize,
) -> Option<NaiveDate> {
    range
        .get((row_idx, 6))
        .and_then(|c| parse_date_or_serial(&c.to_string()).ok())
}

pub(crate) fn is_portfolio_date_header(text: &str) -> bool {
    let t = text.trim();
    t == "data"
        || t.contains("data valoriz")
        || t.contains("data riferimento")
        || t.contains("as of")
}

pub(crate) fn extract_file_statement_date(path: &Path) -> Option<NaiveDate> {
    let metadata = std::fs::metadata(path).ok()?;
    let timestamp = metadata.modified().or_else(|_| metadata.created()).ok()?;
    let local_timestamp: chrono::DateTime<Local> = timestamp.into();
    Some(local_timestamp.naive_local().date())
}

pub(crate) fn determine_transaction_type(account_id: &str, amount: f64) -> (String, String, String) {
    if amount >= 0.0 {
        (
            "income".to_string(),
            "EXTERNAL_PAYER".to_string(),
            account_id.to_string(),
        )
    } else {
        (
            "expense".to_string(),
            account_id.to_string(),
            "EXTERNAL_PAYEE".to_string(),
        )
    }
}

pub(crate) fn make_txn_id(
    prefix: &str,
    date: NaiveDate,
    amount: f64,
    currency: &str,
    description: &str,
) -> String {
    let key = format!(
        "{}|{}|{:.8}|{}|{}",
        prefix,
        date.format("%Y-%m-%d"),
        amount,
        currency.trim(),
        description.trim()
    );
    let hash = make_hash(&key);
    format!("{}-{}", prefix, &hash[..24])
}

pub(crate) fn make_hash(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let hash = hasher.finalize();
    hex::encode(hash)
}

pub(crate) fn intesa_force_expense(desc: &str) -> bool {
    let d = desc.to_lowercase();
    d.contains("canone") || d.contains("spese") || d.contains("imposta")
}

pub fn merge_instruments_with_deduplication(
    template: Value,
    new_instruments: Vec<Value>,
) -> Result<(Value, utils::MergeStats)> {
    utils::merge_instruments_with_deduplication(template, new_instruments)
}
