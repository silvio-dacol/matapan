use anyhow::{Context, Result};
use calamine::{Data, Reader, Xlsx};
use chrono::NaiveDate;
use serde_json::Value;
use utils::{build_transaction, TransactionInput};

use crate::{
    determine_transaction_type, intesa_force_expense, make_txn_id, parse_amount, parse_date_or_serial,
    IntesaSanpaoloParser,
};

pub fn parse_transactions<R: std::io::Read + std::io::Seek>(
    parser: &IntesaSanpaoloParser,
    workbook: &mut Xlsx<R>,
) -> Result<Vec<Value>> {
    let mut all_transactions = Vec::new();

    let sheet_names = workbook.sheet_names().to_vec();
    for sheet_name in sheet_names {
        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            let transactions = parse_transaction_sheet(parser, &range)?;
            all_transactions.extend(transactions);
        }
    }

    Ok(all_transactions)
}

fn parse_transaction_sheet(
    parser: &IntesaSanpaoloParser,
    range: &calamine::Range<Data>,
) -> Result<Vec<Value>> {
    let mut transactions = Vec::new();
    let (height, width) = range.get_size();

    let mut mapping = HeaderMapping::default();

    for row_idx in 0..height.min(30) {
        for col_idx in 0..width {
            if let Some(cell) = range.get((row_idx, col_idx)) {
                mapping.register_header(row_idx, col_idx, &cell.to_string().to_lowercase());
            }
        }

        if mapping.is_complete() {
            break;
        }
    }

    if !mapping.can_parse() {
        return Ok(transactions);
    }

    let header_row = mapping.header_row.unwrap();
    let data_col = mapping.data_col.unwrap();

    for row_idx in (header_row + 1)..height {
        let Some(date_cell) = range.get((row_idx, data_col)) else {
            continue;
        };

        let date_str = date_cell.to_string().trim().to_string();
        if date_str.is_empty() || date_str.to_lowercase().contains("saldo") {
            continue;
        }

        let amount = field_amount(range, row_idx, &mapping)
            .with_context(|| format!("Failed to parse amount at row {}", row_idx))?;

        let date = parse_date_or_serial(&date_str)
            .with_context(|| format!("Failed to parse date: {} at row {}", date_str, row_idx))?;

        let description = field_description(range, row_idx, &mapping);
        let currency = field_currency(range, row_idx, &mapping);
        let account_id = field_account_id(parser, range, row_idx, &mapping, &description);
        let txn_type = field_type(account_id, amount, &description);
        let from_account_id = field_from_account_id(parser, account_id, amount, &description, &txn_type);
        let to_account_id = field_to_account_id(parser, account_id, amount, &description, &txn_type);
        let normalized_amount = field_normalized_amount(amount);

        let row_data = TransactionRowData {
            date,
            from_account_id,
            to_account_id,
            transaction_type: txn_type,
            amount: normalized_amount,
            currency,
            description,
        };

        transactions.push(map_transaction_from_row(&row_data));
    }

    Ok(transactions)
}

#[derive(Default)]
struct HeaderMapping {
    header_row: Option<usize>,
    data_col: Option<usize>,
    operazione_col: Option<usize>,
    dettagli_col: Option<usize>,
    conto_col: Option<usize>,
    valuta_col: Option<usize>,
    importo_col: Option<usize>,
    data_valuta_col: Option<usize>,
    descrizione_col: Option<usize>,
    accrediti_col: Option<usize>,
    addebiti_col: Option<usize>,
}

impl HeaderMapping {
    fn register_header(&mut self, row_idx: usize, col_idx: usize, text: &str) {
        if (text == "data" || text == "data contabile") && self.data_col.is_none() {
            self.data_col = Some(col_idx);
            self.header_row = Some(row_idx);
        }

        if text == "data valuta" && self.data_valuta_col.is_none() {
            self.data_valuta_col = Some(col_idx);
        }

        if text == "operazione" && self.operazione_col.is_none() {
            self.operazione_col = Some(col_idx);
        }

        if text == "descrizione" && self.descrizione_col.is_none() {
            self.descrizione_col = Some(col_idx);
        }

        if (text == "dettagli" || text == "descrizione estesa") && self.dettagli_col.is_none() {
            self.dettagli_col = Some(col_idx);
        }

        if (text.contains("conto") || text.contains("carta") || text == "effettuata tramite:")
            && self.conto_col.is_none()
        {
            self.conto_col = Some(col_idx);
        }

        if text == "valuta" && self.valuta_col.is_none() {
            self.valuta_col = Some(col_idx);
        }

        if text == "importo" && self.importo_col.is_none() {
            self.importo_col = Some(col_idx);
        }

        if text == "accrediti" && self.accrediti_col.is_none() {
            self.accrediti_col = Some(col_idx);
        }

        if text == "addebiti" && self.addebiti_col.is_none() {
            self.addebiti_col = Some(col_idx);
        }
    }

    fn is_complete(&self) -> bool {
        self.data_col.is_some()
            && (self.operazione_col.is_some() || self.descrizione_col.is_some())
            && (self.importo_col.is_some() || (self.accrediti_col.is_some() && self.addebiti_col.is_some()))
    }

    fn can_parse(&mut self) -> bool {
        if self.operazione_col.is_none() && self.descrizione_col.is_some() {
            self.operazione_col = self.descrizione_col;
        }

        if self.data_col.is_none() || self.header_row.is_none() {
            return false;
        }

        self.importo_col.is_some() || (self.accrediti_col.is_some() && self.addebiti_col.is_some())
    }
}

struct TransactionRowData {
    date: NaiveDate,
    from_account_id: String,
    to_account_id: String,
    transaction_type: String,
    amount: f64,
    currency: String,
    description: String,
}

fn map_transaction_from_row(row: &TransactionRowData) -> Value {
    build_transaction(&TransactionInput {
        date: field_date(row),
        from_account_id: field_from(row),
        to_account_id: field_to(row),
        transaction_type: field_transaction_type(row),
        category: field_category(),
        amount: field_amount_abs(row),
        currency: field_currency_out(row),
        description: field_description_out(row),
        description_en: field_description_en(),
        txn_id: field_txn_id(row),
    })
}

fn field_date(row: &TransactionRowData) -> String {
    row.date.format("%Y-%m-%d").to_string()
}

fn field_from(row: &TransactionRowData) -> String {
    row.from_account_id.clone()
}

fn field_to(row: &TransactionRowData) -> String {
    row.to_account_id.clone()
}

fn field_transaction_type(row: &TransactionRowData) -> String {
    row.transaction_type.clone()
}

fn field_category() -> String {
    "uncategorized".to_string()
}

fn field_amount_abs(row: &TransactionRowData) -> f64 {
    row.amount.abs()
}

fn field_currency_out(row: &TransactionRowData) -> String {
    row.currency.clone()
}

fn field_description_out(row: &TransactionRowData) -> String {
    row.description.clone()
}

fn field_description_en() -> Option<String> {
    None
}

fn field_txn_id(row: &TransactionRowData) -> String {
    make_txn_id(
        "INTESA",
        row.date,
        row.amount.abs(),
        &row.currency,
        &row.description,
    )
}

fn field_amount(
    range: &calamine::Range<Data>,
    row_idx: usize,
    mapping: &HeaderMapping,
) -> Result<f64> {
    if let Some(importo_col) = mapping.importo_col {
        let Some(importo_cell) = range.get((row_idx, importo_col)) else {
            anyhow::bail!("missing importo cell");
        };

        let importo_str = importo_cell.to_string().trim().to_string();
        if importo_str.is_empty() {
            anyhow::bail!("empty importo");
        }

        return parse_amount(&importo_str)
            .with_context(|| format!("Failed to parse amount: {}", importo_str));
    }

    let accrediti_str = mapping
        .accrediti_col
        .and_then(|col| range.get((row_idx, col)))
        .map(|c| c.to_string().trim().to_string())
        .unwrap_or_default();

    let addebiti_str = mapping
        .addebiti_col
        .and_then(|col| range.get((row_idx, col)))
        .map(|c| c.to_string().trim().to_string())
        .unwrap_or_default();

    if accrediti_str.is_empty() && addebiti_str.is_empty() {
        anyhow::bail!("both accrediti/addebiti are empty");
    }

    if !accrediti_str.is_empty() {
        parse_amount(&accrediti_str).with_context(|| format!("Failed to parse accrediti: {}", accrediti_str))
    } else {
        let addebiti_amount = parse_amount(&addebiti_str)
            .with_context(|| format!("Failed to parse addebiti: {}", addebiti_str))?;
        Ok(-addebiti_amount)
    }
}

fn field_description(
    range: &calamine::Range<Data>,
    row_idx: usize,
    mapping: &HeaderMapping,
) -> String {
    let mut parts = Vec::new();

    if let Some(col) = mapping.operazione_col {
        if let Some(cell) = range.get((row_idx, col)) {
            let text = cell.to_string().trim().to_string();
            if !text.is_empty() {
                parts.push(text);
            }
        }
    }

    if let Some(col) = mapping.dettagli_col {
        if let Some(cell) = range.get((row_idx, col)) {
            let text = cell.to_string().trim().to_string();
            if !text.is_empty() {
                parts.push(text);
            }
        }
    }

    if parts.is_empty() {
        "Intesa Sanpaolo transaction".to_string()
    } else {
        parts.join(" - ")
    }
}

fn field_currency(
    range: &calamine::Range<Data>,
    row_idx: usize,
    mapping: &HeaderMapping,
) -> String {
    if let Some(col) = mapping.valuta_col {
        let value = range
            .get((row_idx, col))
            .map(|c| c.to_string().trim().to_string())
            .unwrap_or_default();

        if !value.is_empty() {
            return value;
        }
    }

    "EUR".to_string()
}

fn field_account_id<'a>(
    parser: &'a IntesaSanpaoloParser,
    range: &calamine::Range<Data>,
    row_idx: usize,
    mapping: &HeaderMapping,
    description: &str,
) -> &'a str {
    if let Some(col) = mapping.conto_col {
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
            return &parser.account_id_trading;
        }
    }

    &parser.account_id_checking
}

fn field_type(account_id: &str, amount: f64, description: &str) -> String {
    let (mut txn_type, _, _) = determine_transaction_type(account_id, amount);

    if intesa_force_expense(description) {
        txn_type = "expense".to_string();
    }

    txn_type
}

fn field_from_account_id(
    parser: &IntesaSanpaoloParser,
    account_id: &str,
    amount: f64,
    description: &str,
    txn_type: &str,
) -> String {
    let (_, mut from_account, _) = determine_transaction_type(account_id, amount);

    if intesa_force_expense(description) || txn_type == "expense" {
        from_account = parser.account_id_checking.clone();
    }

    from_account
}

fn field_to_account_id(
    _parser: &IntesaSanpaoloParser,
    account_id: &str,
    amount: f64,
    description: &str,
    txn_type: &str,
) -> String {
    let (_, _, mut to_account) = determine_transaction_type(account_id, amount);

    if intesa_force_expense(description) || txn_type == "expense" {
        to_account = "EXTERNAL_PAYEE".to_string();
    }

    to_account
}

fn field_normalized_amount(amount: f64) -> f64 {
    amount.abs()
}
