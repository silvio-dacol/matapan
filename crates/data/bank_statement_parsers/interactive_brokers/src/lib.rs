use anyhow::{anyhow, Context, Result};
use chrono::{Local, NaiveDate};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

mod accounts;
mod instruments;
mod positions;
mod transactions;

pub const PARSER_NAME: &str = "ibkr";

pub const IBKR_CHECKING: &str = "IBKR_CHECKING";
pub const IBKR_SAVINGS: &str = "IBKR_SAVINGS";

#[derive(Debug, Clone)]
pub(crate) struct SectionHeader {
    #[allow(dead_code)]
    cols: Vec<String>,
    idx: HashMap<String, usize>,
}

impl SectionHeader {
    pub(crate) fn new(cols: Vec<String>) -> Self {
        let mut idx = HashMap::new();
        for (i, c) in cols.iter().enumerate() {
            let key = c.trim().to_string();
            if !key.is_empty() {
                idx.insert(key, i);
            }
        }
        Self { cols, idx }
    }

    pub(crate) fn get<'a>(&self, row: &'a [String], col: &str) -> Option<&'a str> {
        let index = *self.idx.get(col)?;
        row.get(index).map(|s| s.as_str())
    }
}

pub struct IbkrCsvParser {
    pub account_id_checking: String,
    pub account_id_savings: String,
}

impl IbkrCsvParser {
    pub fn new() -> Self {
        Self {
            account_id_checking: IBKR_CHECKING.to_string(),
            account_id_savings: IBKR_SAVINGS.to_string(),
        }
    }

    pub fn create_accounts(&self) -> Vec<Value> {
        accounts::create_all_accounts(self)
    }

    pub fn parse_file<P: AsRef<Path>>(&self, path: P) -> Result<ParsedIbkr> {
        let fallback_statement_date = extract_statement_end_from_filename(path.as_ref())
            .or_else(|| extract_file_statement_date(path.as_ref()))
            .unwrap_or_else(|| Local::now().naive_local().date());

        let mut file = std::fs::File::open(path.as_ref())
            .with_context(|| format!("Cannot open {}", path.as_ref().display()))?;

        self.parse_reader_with_fallback_date(&mut file, Some(fallback_statement_date))
    }

    pub fn parse_reader<R: Read>(&self, reader: R) -> Result<ParsedIbkr> {
        self.parse_reader_with_fallback_date(reader, None)
    }

    fn parse_reader_with_fallback_date<R: Read>(
        &self,
        mut reader: R,
        fallback_statement_date: Option<NaiveDate>,
    ) -> Result<ParsedIbkr> {
        let mut buf = String::new();
        reader.read_to_string(&mut buf)?;

        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(buf.as_bytes());

        let mut headers: HashMap<String, SectionHeader> = HashMap::new();
        let mut statement_end: Option<NaiveDate> = None;

        let mut instruments_out: Vec<Value> = Vec::new();
        let mut positions_out: Vec<Value> = Vec::new();
        let mut transactions_out: Vec<Value> = Vec::new();

        let mut instrument_key_to_id: HashMap<String, String> = HashMap::new();
        let mut instrument_id_remap: HashMap<String, String> = HashMap::new();

        for record in csv_reader.records() {
            let record = record?;
            if record.len() < 2 {
                continue;
            }

            let section = record.get(0).unwrap_or("").trim();
            let row_kind = record.get(1).unwrap_or("").trim();

            let row: Vec<String> = (2..record.len())
                .map(|index| record.get(index).unwrap_or("").to_string())
                .collect();

            if row_kind == "Header" {
                let columns = row.iter().map(|s| s.trim().to_string()).collect::<Vec<_>>();
                headers.insert(section.to_string(), SectionHeader::new(columns));
                continue;
            }

            if row_kind != "Data" {
                continue;
            }

            if section.eq_ignore_ascii_case("Statement") {
                update_statement_end(&row, &mut statement_end);
                continue;
            }

            if instruments::parse_instrument_row(
                section,
                &headers,
                &row,
                &mut instruments_out,
                &mut instrument_key_to_id,
                &mut instrument_id_remap,
            )? {
                continue;
            }

            if positions::parse_position_row(
                self,
                section,
                &headers,
                &row,
                statement_end,
                fallback_statement_date,
                &mut positions_out,
                &mut instruments_out,
                &mut instrument_key_to_id,
            )? {
                continue;
            }

            let _ = transactions::parse_transaction_row(
                self,
                section,
                &headers,
                &row,
                &mut transactions_out,
            )?;
        }

        positions::reconcile_position_instrument_ids(
            &mut positions_out,
            &self.account_id_savings,
            &instrument_id_remap,
        );

        Ok(ParsedIbkr {
            statement_end,
            instruments: instruments_out,
            positions: positions_out,
            transactions: transactions_out,
        })
    }
}

impl Default for IbkrCsvParser {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ParsedIbkr {
    pub statement_end: Option<NaiveDate>,
    pub instruments: Vec<Value>,
    pub positions: Vec<Value>,
    pub transactions: Vec<Value>,
}

pub fn merge_instruments_with_deduplication(
    template: Value,
    new_instruments: Vec<Value>,
) -> Result<(Value, utils::MergeStats)> {
    utils::merge_instruments_with_deduplication(template, new_instruments)
}

fn update_statement_end(row: &[String], statement_end: &mut Option<NaiveDate>) {
    let field_name = row
        .first()
        .map(|s| s.trim())
        .unwrap_or("")
        .to_lowercase();
    let field_value = row.get(1).map(|s| s.trim()).unwrap_or("");

    if field_name.contains("whengenerated") {
        *statement_end = parse_statement_generated(field_value).ok().or(*statement_end);
    }
    if field_name.contains("period") {
        *statement_end = parse_statement_end(field_value).ok();
    }
}

pub(crate) fn null_if_empty_string(s: &str) -> Option<String> {
    let text = s.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn parse_statement_end(period: &str) -> Result<NaiveDate> {
    let normalized = period
        .trim()
        .trim_matches('"')
        .replace('–', "-")
        .replace('—', "-");

    if let Some((_, end)) = normalized.split_once('-') {
        return parse_flexible_date(end.trim());
    }

    parse_flexible_date(&normalized)
}

fn parse_statement_generated(value: &str) -> Result<NaiveDate> {
    parse_ibkr_datetime_date(value)
}

fn extract_file_statement_date(path: &Path) -> Option<NaiveDate> {
    let metadata = std::fs::metadata(path).ok()?;
    let timestamp = metadata.modified().or_else(|_| metadata.created()).ok()?;
    let local_timestamp: chrono::DateTime<Local> = timestamp.into();
    Some(local_timestamp.naive_local().date())
}

fn extract_statement_end_from_filename(path: &Path) -> Option<NaiveDate> {
    let stem = path.file_stem()?.to_string_lossy();
    let last_token = stem.split('_').next_back()?;

    if last_token.len() != 8 || !last_token.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    NaiveDate::parse_from_str(last_token, "%Y%m%d").ok()
}

pub(crate) fn parse_yyyy_mm_dd(s: &str) -> Result<NaiveDate> {
    Ok(NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")?)
}

fn parse_flexible_date(s: &str) -> Result<NaiveDate> {
    let text = s.trim().trim_matches('"');

    if let Ok(date) = NaiveDate::parse_from_str(text, "%B %d, %Y") {
        return Ok(date);
    }
    if let Ok(date) = NaiveDate::parse_from_str(text, "%b %d, %Y") {
        return Ok(date);
    }
    if let Ok(date) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return Ok(date);
    }
    if let Ok(date) = NaiveDate::parse_from_str(text, "%m/%d/%Y") {
        return Ok(date);
    }

    Err(anyhow!("Unsupported date format: {}", s))
}

pub(crate) fn parse_ibkr_datetime_date(s: &str) -> Result<NaiveDate> {
    let text = s.trim();
    let date_part = text.split(',').next().unwrap_or(text).trim();
    parse_yyyy_mm_dd(date_part)
}

pub(crate) fn parse_f64(s: &str) -> Result<f64> {
    let text = s.trim();
    if text.is_empty() || text == "--" {
        return Err(anyhow!("empty/NA number"));
    }
    let normalized = text.replace(',', "");
    Ok(normalized.parse::<f64>()?)
}

pub(crate) fn parse_f64_opt(s: &str) -> Option<f64> {
    parse_f64(s).ok()
}

pub(crate) fn cashflow_to_txn(account_id: &str, signed: f64) -> (String, String, String, f64) {
    if signed >= 0.0 {
        (
            "income".to_string(),
            "EXTERNAL_PAYER".to_string(),
            account_id.to_string(),
            signed.abs(),
        )
    } else {
        (
            "expense".to_string(),
            account_id.to_string(),
            "EXTERNAL_PAYEE".to_string(),
            signed.abs(),
        )
    }
}

pub(crate) fn build_instrument_id(
    conid: &str,
    security_id: &str,
    asset_category: &str,
    symbol: &str,
) -> String {
    let sid = security_id.trim();
    if looks_like_isin(sid) {
        return format!("IBKR_{}", sid);
    }

    let trimmed_conid = conid.trim();
    if !trimmed_conid.is_empty() {
        return format!("IBKR_CONID_{}", trimmed_conid);
    }

    let category = asset_category.trim().replace(' ', "_");
    let ticker = symbol.trim().replace(' ', "_");
    format!("IBKR_{}_{}", category, ticker)
}

pub(crate) fn split_symbol_aliases(symbol: &str) -> Vec<String> {
    let mut aliases: Vec<String> = Vec::new();
    for part in symbol.split(',') {
        let alias = part.trim();
        if !alias.is_empty() && !aliases.iter().any(|a| a == alias) {
            aliases.push(alias.to_string());
        }
    }
    aliases
}

pub(crate) fn primary_symbol(symbol: &str) -> String {
    split_symbol_aliases(symbol)
        .into_iter()
        .next()
        .unwrap_or_else(|| symbol.trim().to_string())
}

pub(crate) fn resolve_instrument_id(id: &str, remap: &HashMap<String, String>) -> String {
    let mut current = id.to_string();
    while let Some(next) = remap.get(&current) {
        if *next == current {
            break;
        }
        current = next.clone();
    }
    current
}

pub(crate) fn merge_instrument_entries(existing: &Value, incoming: &Value) -> Value {
    let existing_score = instrument_completeness_score(existing);
    let incoming_score = instrument_completeness_score(incoming);
    if incoming_score >= existing_score {
        incoming.clone()
    } else {
        existing.clone()
    }
}

fn instrument_completeness_score(inst: &Value) -> usize {
    ["description", "security_id", "type"]
        .iter()
        .filter(|field| match inst.get(**field) {
            Some(Value::Null) | None => false,
            Some(Value::String(s)) => !s.trim().is_empty(),
            Some(Value::Number(_)) => true,
            _ => true,
        })
        .count()
}

fn looks_like_isin(s: &str) -> bool {
    if s.len() != 12 {
        return false;
    }
    s.chars().all(|c| c.is_ascii_alphanumeric())
}

pub(crate) fn build_txn_id(
    prefix: &str,
    date: NaiveDate,
    amount: f64,
    currency: &str,
    description: &str,
    extra: &str,
) -> String {
    let key = format!(
        "{}|{}|{:.8}|{}|{}|{}",
        prefix,
        date.format("%Y-%m-%d"),
        amount,
        currency.trim(),
        description.trim(),
        extra.trim()
    );
    let hash = make_hash_id(&key);
    format!("{}-{}", prefix, &hash[..24])
}

pub(crate) fn make_hash_id(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let hash = hasher.finalize();
    hex::encode(hash)
}
