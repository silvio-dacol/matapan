use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook, Reader, Xlsx};
use chrono::{Datelike, NaiveDate};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const PARSER_NAME: &str = "intesa_sanpaolo";

pub const INTESA_CHECKING: &str = "INTESA_CHECKING";
pub const INTESA_SAVINGS: &str = "INTESA_SAVINGS";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileType {
    Transactions, // Lista Operazione - transaction list
    Portfolio,    // Patrimonio - holdings/positions
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

    /// Creates account entries for both Intesa Sanpaolo accounts
    pub fn create_accounts(&self) -> Vec<Value> {
        vec![
            json!({
                "account_id": self.account_id_checking,
                "structural_type": "bank",
                "institution": "Intesa Sanpaolo",
                "country": "IT",
                "iban": null,
                "bic": null,
                "account_number": null,
                "owner": "self",
                "is_liability": false,
                "supports_positions": false,
                "opened_date": null,
                "closed_date": null,
                "is_active": true,
                "notes": "Intesa Sanpaolo checking account"
            }),
            json!({
                "account_id": self.account_id_trading,
                "structural_type": "brokerage",
                "institution": "Intesa Sanpaolo",
                "country": "IT",
                "iban": null,
                "bic": null,
                "account_number": null,
                "owner": "self",
                "is_liability": false,
                "supports_positions": true,
                "opened_date": null,
                "closed_date": null,
                "is_active": true,
                "notes": "Intesa Sanpaolo trading/investment account"
            }),
        ]
    }

    /// Detects the file type based on file content
    pub fn detect_file_type<P: AsRef<Path>>(&self, path: P) -> Result<FileType> {
        let path_str = path.as_ref().to_string_lossy().to_lowercase();

        // Check filename patterns first
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

        // Try to open and inspect the file
        let mut workbook: Xlsx<_> = open_workbook(&path)
            .with_context(|| format!("Failed to open workbook: {}", path.as_ref().display()))?;

        // Look for sheet names that indicate file type
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

        // Look at first sheet content
        if let Some(first_sheet) = sheet_names.first() {
            if let Ok(range) = workbook.worksheet_range(first_sheet) {
                // Check for ISIN column which indicates portfolio
                for row_idx in 0..range.get_size().0.min(15) {
                    for col_idx in 0..range.get_size().1 {
                        if let Some(cell) = range.get((row_idx, col_idx)) {
                            let text = cell.to_string().to_lowercase();
                            if text == "isin" {
                                return Ok(FileType::Portfolio);
                            }
                            if text.contains("importo") && (text.contains("data") || row_idx < 3) {
                                // Check if other columns suggest transaction list
                                for check_col in 0..range.get_size().1 {
                                    if let Some(check_cell) = range.get((row_idx, check_col)) {
                                        let check_text = check_cell.to_string().to_lowercase();
                                        if check_text.contains("operazione") || check_text == "data"
                                        {
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

        // Default to transactions if unclear
        println!(
            "⚠️  Could not determine file type for {}, defaulting to transactions",
            path.as_ref().display()
        );
        Ok(FileType::Transactions)
    }

    /// Parse a file and automatically detect its type
    pub fn parse_file<P: AsRef<Path>>(&self, path: P) -> Result<ParsedIntesa> {
        let file_type = self.detect_file_type(&path)?;
        self.parse_file_with_type(&path, file_type)
    }

    /// Parse a file with explicit file type
    pub fn parse_file_with_type<P: AsRef<Path>>(
        &self,
        path: P,
        file_type: FileType,
    ) -> Result<ParsedIntesa> {
        let mut workbook: Xlsx<_> = open_workbook(&path)
            .with_context(|| format!("Failed to open workbook: {}", path.as_ref().display()))?;

        match file_type {
            FileType::Transactions => {
                let transactions = self.parse_transactions(&mut workbook)?;
                Ok(ParsedIntesa {
                    transactions,
                    positions: Vec::new(),
                    instruments: Vec::new(),
                    file_type,
                })
            }
            FileType::Portfolio => {
                let (instruments, positions) = self.parse_portfolio(&mut workbook)?;
                Ok(ParsedIntesa {
                    transactions: Vec::new(),
                    positions,
                    instruments,
                    file_type,
                })
            }
        }
    }

    /// Parse transaction list (Lista Operazione sheet)
    fn parse_transactions<R: std::io::Read + std::io::Seek>(
        &self,
        workbook: &mut Xlsx<R>,
    ) -> Result<Vec<Value>> {
        let mut all_transactions = Vec::new();

        let sheet_names = workbook.sheet_names().to_vec();
        for sheet_name in sheet_names {
            if let Ok(range) = workbook.worksheet_range(&sheet_name) {
                let transactions = self.parse_transaction_sheet(&range)?;
                all_transactions.extend(transactions);
            }
        }

        Ok(all_transactions)
    }

    fn parse_transaction_sheet(
        &self,
        range: &calamine::Range<calamine::Data>,
    ) -> Result<Vec<Value>> {
        let mut transactions = Vec::new();
        let (height, width) = range.get_size();

        // Find header row - look for "Data", "Operazione", "Importo"
        let mut header_row = None;
        let mut data_col = None;
        let mut operazione_col = None;
        let mut dettagli_col = None;
        let mut conto_col = None;
        let mut valuta_col = None;
        let mut importo_col = None;

        for row_idx in 0..height.min(20) {
            for col_idx in 0..width {
                if let Some(cell) = range.get((row_idx, col_idx)) {
                    let text = cell.to_string().to_lowercase();
                    if text == "data" && data_col.is_none() {
                        data_col = Some(col_idx);
                        header_row = Some(row_idx);
                    }
                    if text == "operazione" && operazione_col.is_none() {
                        operazione_col = Some(col_idx);
                    }
                    if text == "dettagli" && dettagli_col.is_none() {
                        dettagli_col = Some(col_idx);
                    }
                    if (text.contains("conto") || text.contains("carta")) && conto_col.is_none() {
                        conto_col = Some(col_idx);
                    }
                    if text == "valuta" && valuta_col.is_none() {
                        valuta_col = Some(col_idx);
                    }
                    if text == "importo" && importo_col.is_none() {
                        importo_col = Some(col_idx);
                    }
                }
            }
            if data_col.is_some() && operazione_col.is_some() && importo_col.is_some() {
                break;
            }
        }

        if data_col.is_none() || importo_col.is_none() || header_row.is_none() {
            // No valid transaction data found
            return Ok(transactions);
        }

        let header_row = header_row.unwrap();
        let data_col = data_col.unwrap();
        let importo_col = importo_col.unwrap();

        // Parse data rows
        for row_idx in (header_row + 1)..height {
            let date_cell = range.get((row_idx, data_col));
            let importo_cell = range.get((row_idx, importo_col));

            if date_cell.is_none() || importo_cell.is_none() {
                continue;
            }

            let date_str = date_cell.unwrap().to_string().trim().to_string();
            let importo_str = importo_cell.unwrap().to_string().trim().to_string();

            if date_str.is_empty() || importo_str.is_empty() {
                continue;
            }

            // Parse date (could be Excel serial date or formatted string)
            let date = parse_date_or_serial(&date_str).with_context(|| {
                format!("Failed to parse date: {} at row {}", date_str, row_idx)
            })?;

            // Parse amount
            let amount = parse_amount(&importo_str).with_context(|| {
                format!("Failed to parse amount: {} at row {}", importo_str, row_idx)
            })?;

            // Build description from available columns
            let mut desc_parts = Vec::new();

            if let Some(col) = operazione_col {
                if let Some(cell) = range.get((row_idx, col)) {
                    let text = cell.to_string().trim().to_string();
                    if !text.is_empty() {
                        desc_parts.push(text);
                    }
                }
            }

            if let Some(col) = dettagli_col {
                if let Some(cell) = range.get((row_idx, col)) {
                    let text = cell.to_string().trim().to_string();
                    if !text.is_empty() {
                        desc_parts.push(text);
                    }
                }
            }

            let description = if desc_parts.is_empty() {
                "Intesa Sanpaolo transaction".to_string()
            } else {
                desc_parts.join(" - ")
            };

            // Get currency
            let currency = if let Some(col) = valuta_col {
                range
                    .get((row_idx, col))
                    .map(|c| c.to_string().trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "EUR".to_string())
            } else {
                "EUR".to_string()
            };

            // Determine account - check if this is trading related
            let account_id = if let Some(col) = conto_col {
                let conto_text = range
                    .get((row_idx, col))
                    .map(|c| c.to_string().to_lowercase())
                    .unwrap_or_default();
                if conto_text.contains("deposito")
                    || conto_text.contains("amministrato")
                    || description.to_lowercase().contains("titoli")
                    || description.to_lowercase().contains("cedole")
                    || description.to_lowercase().contains("compravendita")
                {
                    &self.account_id_trading
                } else {
                    &self.account_id_checking
                }
            } else {
                &self.account_id_checking
            };

            let (txn_type, from_account, to_account) =
                determine_transaction_type(account_id, amount);

            let txn_id = make_txn_id("INTESA", date, amount.abs(), &currency, &description);

            transactions.push(json!({
                "date": date.format("%Y-%m-%d").to_string(),
                "from_account_id": from_account,
                "to_account_id": to_account,
                "type": txn_type,
                "category": "uncategorized",
                "amount": amount.abs(),
                "currency": currency,
                "description": description,
                "txn_id": txn_id
            }));
        }

        Ok(transactions)
    }

    /// Parse portfolio/patrimonio sheet
    fn parse_portfolio<R: std::io::Read + std::io::Seek>(
        &self,
        workbook: &mut Xlsx<R>,
    ) -> Result<(Vec<Value>, Vec<Value>)> {
        let mut instruments = Vec::new();
        let mut positions = Vec::new();

        let sheet_names = workbook.sheet_names().to_vec();
        for sheet_name in sheet_names {
            if let Ok(range) = workbook.worksheet_range(&sheet_name) {
                let (inst, pos) = self.parse_portfolio_sheet(&range)?;
                instruments.extend(inst);
                positions.extend(pos);
            }
        }

        Ok((instruments, positions))
    }

    fn parse_portfolio_sheet(
        &self,
        range: &calamine::Range<calamine::Data>,
    ) -> Result<(Vec<Value>, Vec<Value>)> {
        let mut instruments = Vec::new();
        let mut positions = Vec::new();
        let (height, width) = range.get_size();

        // Find the as_of_date from the sheet (usually in top rows)
        let as_of_date = extract_portfolio_date(range)
            .unwrap_or_else(|| chrono::Local::now().naive_local().date());

        // Find header row with ISIN, Quantità, etc.
        let mut header_row = None;
        let mut descrizione_col = None;
        let mut isin_col = None;
        let mut quantita_col = None;
        let mut prezzo_carico_col = None;
        let mut prezzo_mercato_col = None;
        let mut controvalore_col = None;
        let mut valore_carico_col = None;

        for row_idx in 0..height.min(20) {
            for col_idx in 0..width {
                if let Some(cell) = range.get((row_idx, col_idx)) {
                    let text = cell.to_string().to_lowercase();
                    if text == "isin" {
                        isin_col = Some(col_idx);
                        header_row = Some(row_idx);
                    }
                    if text == "descrizione" && descrizione_col.is_none() {
                        descrizione_col = Some(col_idx);
                    }
                    if text.contains("quantit") && quantita_col.is_none() {
                        quantita_col = Some(col_idx);
                    }
                    if text.contains("prezzo medio") && prezzo_carico_col.is_none() {
                        prezzo_carico_col = Some(col_idx);
                    }
                    if text.contains("prezzo mercato") && prezzo_mercato_col.is_none() {
                        prezzo_mercato_col = Some(col_idx);
                    }
                    if text == "controvalore €"
                        || (text.contains("controvalore") && !text.contains("totale"))
                    {
                        if controvalore_col.is_none() {
                            controvalore_col = Some(col_idx);
                        }
                    }
                    if text.contains("valore carico") && valore_carico_col.is_none() {
                        valore_carico_col = Some(col_idx);
                    }
                }
            }
            if isin_col.is_some() && quantita_col.is_some() {
                break;
            }
        }

        if isin_col.is_none() || header_row.is_none() {
            // No valid portfolio data
            return Ok((instruments, positions));
        }

        let header_row = header_row.unwrap();
        let isin_col = isin_col.unwrap();

        // Parse positions
        for row_idx in (header_row + 1)..height {
            let isin_cell = range.get((row_idx, isin_col));

            if isin_cell.is_none() {
                continue;
            }

            let isin = isin_cell.unwrap().to_string().trim().to_string();

            // Skip if ISIN is empty or looks like a total/summary row
            if isin.is_empty()
                || !isin.starts_with(|c: char| c.is_ascii_alphabetic())
                || isin.len() != 12
            {
                continue;
            }

            let description = descrizione_col
                .and_then(|col| range.get((row_idx, col)))
                .map(|c| c.to_string().trim().to_string())
                .unwrap_or_else(|| "".to_string());

            let quantity = quantita_col
                .and_then(|col| range.get((row_idx, col)))
                .and_then(|c| parse_amount(&c.to_string()).ok());

            let cost_price = prezzo_carico_col
                .and_then(|col| range.get((row_idx, col)))
                .and_then(|c| parse_amount(&c.to_string()).ok());

            let market_price = prezzo_mercato_col
                .and_then(|col| range.get((row_idx, col)))
                .and_then(|c| parse_amount(&c.to_string()).ok());

            let market_value = controvalore_col
                .and_then(|col| range.get((row_idx, col)))
                .and_then(|c| parse_amount(&c.to_string()).ok());

            let cost_basis = valore_carico_col
                .and_then(|col| range.get((row_idx, col)))
                .and_then(|c| parse_amount(&c.to_string()).ok());

            if quantity.is_none() || quantity == Some(0.0) {
                continue;
            }

            // Create instrument
            let instrument_id = format!("ISIN_{}", isin);
            let instrument = json!({
                "instrument_id": instrument_id,
                "source": "Intesa Sanpaolo",
                "asset_category": null,
                "symbol": null,
                "description": if description.is_empty() { Value::Null } else { Value::String(description.clone()) },
                "conid": null,
                "security_id": isin.clone(),
                "listing_exchange": null,
                "type": null,
                "underlying": null,
                "multiplier": null
            });

            instruments.push(instrument);

            // Create position
            let position_id = make_hash(&format!(
                "{}|{}|{}",
                self.account_id_trading,
                as_of_date.format("%Y-%m-%d"),
                instrument_id
            ));

            let unrealized_pnl = if let (Some(mv), Some(cb)) = (market_value, cost_basis) {
                Some(mv - cb)
            } else {
                None
            };

            positions.push(json!({
                "position_id": format!("INTESAPOS-{}", &position_id[..12]),
                "source": "Intesa Sanpaolo",
                "as_of_date": as_of_date.format("%Y-%m-%d").to_string(),
                "account_id": self.account_id_trading,
                "instrument_id": instrument_id,
                "quantity": quantity,
                "currency": "EUR",
                "cost_price": cost_price,
                "cost_basis": cost_basis,
                "close_price": market_price,
                "market_value": market_value,
                "unrealized_pnl": unrealized_pnl
            }));
        }

        Ok((instruments, positions))
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

// Helper functions

fn parse_date_or_serial(s: &str) -> Result<NaiveDate> {
    let s = s.trim();

    // Try to parse as Excel serial date (number like 45991)
    if let Ok(serial) = s.parse::<f64>() {
        // Excel dates are days since 1899-12-30 (with 1900 leap year bug)
        if serial >= 1.0 && serial < 100000.0 {
            let days = serial.floor() as i64;
            // Excel epoch: 1899-12-30
            let base_date = NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
            if let Some(date) = base_date.checked_add_signed(chrono::Duration::days(days)) {
                return Ok(date);
            }
        }
    }

    // Try various text date formats
    parse_date(s)
}

fn parse_date(s: &str) -> Result<NaiveDate> {
    let s = s.trim();

    // DD/MM/YYYY
    if let Ok(date) = NaiveDate::parse_from_str(s, "%d/%m/%Y") {
        return Ok(date);
    }

    // DD-MM-YYYY
    if let Ok(date) = NaiveDate::parse_from_str(s, "%d-%m-%Y") {
        return Ok(date);
    }

    // YYYY-MM-DD
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(date);
    }

    // DD.MM.YYYY
    if let Ok(date) = NaiveDate::parse_from_str(s, "%d.%m.%Y") {
        return Ok(date);
    }

    Err(anyhow!("Unrecognized date format: {}", s))
}

fn parse_amount(s: &str) -> Result<f64> {
    let s = s.trim();

    // Handle empty or placeholder strings
    if s.is_empty() || s == "-" || s == "--" {
        return Err(anyhow!("Empty amount"));
    }

    // Remove currency symbols and spaces
    let mut cleaned = s.to_string();
    cleaned = cleaned.replace("€", "").replace("EUR", "").replace(" ", "");

    // Handle Italian/European number format (dots for thousands, comma for decimal)
    // Example: "1.234,56" or "-1.234,56"
    if cleaned.contains(',') {
        // Remove dots (thousands separator) and replace comma with dot (decimal)
        cleaned = cleaned.replace('.', "");
        cleaned = cleaned.replace(',', ".");
    }

    // Parse the number
    cleaned
        .parse::<f64>()
        .with_context(|| format!("Failed to parse amount: {}", s))
}

fn extract_portfolio_date(range: &calamine::Range<calamine::Data>) -> Option<NaiveDate> {
    // Look for dates in the first few rows
    let (height, width) = range.get_size();
    for row_idx in 0..height.min(10) {
        for col_idx in 0..width {
            if let Some(cell) = range.get((row_idx, col_idx)) {
                let text = cell.to_string();
                // Try to parse as date
                if let Ok(date) = parse_date_or_serial(&text) {
                    // Check if it's a reasonable recent date
                    if date.year() >= 2020 && date.year() <= 2030 {
                        return Some(date);
                    }
                }
            }
        }
    }
    None
}

fn determine_transaction_type(account_id: &str, amount: f64) -> (String, String, String) {
    if amount >= 0.0 {
        // Money coming in
        (
            "income".to_string(),
            "EXTERNAL_PAYER".to_string(),
            account_id.to_string(),
        )
    } else {
        // Money going out
        (
            "expense".to_string(),
            account_id.to_string(),
            "EXTERNAL_PAYEE".to_string(),
        )
    }
}

fn make_txn_id(
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

fn make_hash(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let hash = hasher.finalize();
    hex::encode(hash)
}

// ------------------------
// Merging helpers
// ------------------------

pub fn merge_instruments_with_deduplication(
    mut template: Value,
    new_instruments: Vec<Value>,
) -> Result<(Value, utils::MergeStats)> {
    use std::collections::HashSet;

    let arr = template
        .get_mut("instruments")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("database.json missing 'instruments' array"))?;

    let existing: HashSet<String> = arr
        .iter()
        .filter_map(|v| {
            v.get("instrument_id")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let mut stats = utils::MergeStats {
        added: 0,
        skipped: 0,
        total: new_instruments.len(),
    };

    for inst in new_instruments {
        let id = inst
            .get("instrument_id")
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow!("Instrument missing instrument_id"))?;

        if existing.contains(id) {
            stats.skipped += 1;
        } else {
            arr.push(inst);
            stats.added += 1;
        }
    }

    Ok((template, stats))
}

pub fn merge_positions_with_deduplication(
    mut template: Value,
    new_positions: Vec<Value>,
) -> Result<(Value, utils::MergeStats)> {
    use std::collections::HashSet;

    let arr = template
        .get_mut("positions")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("database.json missing 'positions' array"))?;

    let existing: HashSet<String> = arr
        .iter()
        .filter_map(|v| {
            v.get("position_id")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let mut stats = utils::MergeStats {
        added: 0,
        skipped: 0,
        total: new_positions.len(),
    };

    for pos in new_positions {
        let id = pos
            .get("position_id")
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow!("Position missing position_id"))?;

        if existing.contains(id) {
            stats.skipped += 1;
        } else {
            arr.push(pos);
            stats.added += 1;
        }
    }

    Ok((template, stats))
}
