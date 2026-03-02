use anyhow::Result;
use calamine::{Data, Reader, Xlsx};
use chrono::NaiveDate;
use serde_json::Value;
use utils::{build_position, PositionInput};

use crate::instruments::{map_instrument_from_row, PortfolioRowData};
use crate::{
    extract_portfolio_date, extract_row_date_from_column_g, is_portfolio_date_header, make_hash,
    parse_amount, parse_date_or_serial, IntesaSanpaoloParser,
};

pub fn parse_portfolio<R: std::io::Read + std::io::Seek>(
    parser: &IntesaSanpaoloParser,
    workbook: &mut Xlsx<R>,
    fallback_statement_date: NaiveDate,
) -> Result<(Vec<Value>, Vec<Value>)> {
    let mut instruments = Vec::new();
    let mut positions = Vec::new();

    let sheet_names = workbook.sheet_names().to_vec();
    for sheet_name in sheet_names {
        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            let (sheet_instruments, sheet_positions) =
                parse_portfolio_sheet(parser, &range, fallback_statement_date)?;
            instruments.extend(sheet_instruments);
            positions.extend(sheet_positions);
        }
    }

    Ok((instruments, positions))
}

fn parse_portfolio_sheet(
    parser: &IntesaSanpaoloParser,
    range: &calamine::Range<Data>,
    fallback_statement_date: NaiveDate,
) -> Result<(Vec<Value>, Vec<Value>)> {
    let mut instruments = Vec::new();
    let mut positions = Vec::new();
    let (height, width) = range.get_size();

    let as_of_date = extract_portfolio_date(range).unwrap_or(fallback_statement_date);

    let mut header_row = None;
    let mut descrizione_col = None;
    let mut isin_col = None;
    let mut quantita_col = None;
    let mut prezzo_carico_col = None;
    let mut prezzo_mercato_col = None;
    let mut controvalore_col = None;
    let mut valore_carico_col = None;
    let mut as_of_col = None;

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
                if text == "controvalore €" || (text.contains("controvalore") && !text.contains("totale")) {
                    controvalore_col = Some(col_idx);
                }
                if text.contains("valore carico") && valore_carico_col.is_none() {
                    valore_carico_col = Some(col_idx);
                }
                if is_portfolio_date_header(&text) && as_of_col.is_none() {
                    as_of_col = Some(col_idx);
                }
            }
        }

        if isin_col.is_some() && quantita_col.is_some() {
            break;
        }
    }

    if isin_col.is_none() || header_row.is_none() {
        return Ok((instruments, positions));
    }

    let header_row = header_row.unwrap();
    let isin_col = isin_col.unwrap();

    for row_idx in (header_row + 1)..height {
        let Some(isin_cell) = range.get((row_idx, isin_col)) else {
            continue;
        };

        let isin = isin_cell.to_string().trim().to_string();
        if isin.is_empty() || !isin.starts_with(|c: char| c.is_ascii_alphabetic()) || isin.len() != 12 {
            continue;
        }

        let row_data = PortfolioMappedRow {
            isin: isin.clone(),
            description: descrizione_col
                .and_then(|col| range.get((row_idx, col)))
                .map(|c| c.to_string().trim().to_string())
                .unwrap_or_default(),
            quantity: quantita_col
                .and_then(|col| range.get((row_idx, col)))
                .and_then(|c| parse_amount(&c.to_string()).ok()),
            cost_price: prezzo_carico_col
                .and_then(|col| range.get((row_idx, col)))
                .and_then(|c| parse_amount(&c.to_string()).ok()),
            close_price: prezzo_mercato_col
                .and_then(|col| range.get((row_idx, col)))
                .and_then(|c| parse_amount(&c.to_string()).ok()),
            market_value: controvalore_col
                .and_then(|col| range.get((row_idx, col)))
                .and_then(|c| parse_amount(&c.to_string()).ok()),
            cost_basis: valore_carico_col
                .and_then(|col| range.get((row_idx, col)))
                .and_then(|c| parse_amount(&c.to_string()).ok()),
            position_as_of_date: as_of_col
                .and_then(|col| range.get((row_idx, col)))
                .and_then(|c| parse_date_or_serial(&c.to_string()).ok())
                .or_else(|| extract_row_date_from_column_g(range, row_idx))
                .unwrap_or(as_of_date),
        };

        if row_data.quantity.is_none() || row_data.quantity == Some(0.0) {
            continue;
        }

        let instrument_row = PortfolioRowData {
            isin: row_data.isin.clone(),
            description: row_data.description.clone(),
        };

        if let Some(instrument) = map_instrument_from_row(&instrument_row) {
            instruments.push(instrument);
        }

        if let Some(position) = map_position_from_row(parser, &row_data) {
            positions.push(position);
        }
    }

    Ok((instruments, positions))
}

struct PortfolioMappedRow {
    isin: String,
    description: String,
    quantity: Option<f64>,
    cost_price: Option<f64>,
    cost_basis: Option<f64>,
    close_price: Option<f64>,
    market_value: Option<f64>,
    position_as_of_date: NaiveDate,
}

fn map_position_from_row(parser: &IntesaSanpaoloParser, row: &PortfolioMappedRow) -> Option<Value> {
    let position_id = field_position_id(parser, row)?;

    Some(build_position(
        &PositionInput {
            position_id,
            source: field_source(),
            as_of_date: field_as_of_date(row),
            account_id: field_account_id(parser),
            instrument_id: field_instrument_id(row),
            quantity: field_quantity(row),
            currency: field_currency(),
            cost_price: field_cost_price(row),
            cost_basis: field_cost_basis(row),
            close_price: field_close_price(row),
            market_value: field_market_value(row),
        },
        field_unrealized_pnl(row),
    ))
}

fn field_position_id(parser: &IntesaSanpaoloParser, row: &PortfolioMappedRow) -> Option<String> {
    let instrument_id = field_instrument_id(row);
    let key = format!(
        "{}|{}|{}",
        parser.account_id_trading,
        row.position_as_of_date.format("%Y-%m-%d"),
        instrument_id
    );
    let hash = make_hash(&key);
    Some(format!("INTESAPOS-{}", &hash[..12]))
}

fn field_source() -> String {
    "Intesa Sanpaolo".to_string()
}

fn field_as_of_date(row: &PortfolioMappedRow) -> String {
    row.position_as_of_date.format("%Y-%m-%d").to_string()
}

fn field_account_id(parser: &IntesaSanpaoloParser) -> String {
    parser.account_id_trading.clone()
}

fn field_instrument_id(row: &PortfolioMappedRow) -> String {
    format!("ISIN_{}", row.isin)
}

fn field_quantity(row: &PortfolioMappedRow) -> Option<f64> {
    row.quantity
}

fn field_currency() -> Option<String> {
    Some("EUR".to_string())
}

fn field_cost_price(row: &PortfolioMappedRow) -> Option<f64> {
    row.cost_price
}

fn field_cost_basis(row: &PortfolioMappedRow) -> Option<f64> {
    row.cost_basis
}

fn field_close_price(row: &PortfolioMappedRow) -> Option<f64> {
    row.close_price
}

fn field_market_value(row: &PortfolioMappedRow) -> Option<f64> {
    row.market_value
}

fn field_unrealized_pnl(row: &PortfolioMappedRow) -> Option<f64> {
    if let (Some(market_value), Some(cost_basis)) = (row.market_value, row.cost_basis) {
        Some(market_value - cost_basis)
    } else {
        None
    }
}
