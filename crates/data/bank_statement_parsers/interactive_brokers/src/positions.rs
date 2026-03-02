use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use serde_json::{json, Value};
use std::collections::HashMap;
use utils::{build_position, PositionInput};

use crate::{
    build_instrument_id, make_hash_id, null_if_empty_string, parse_f64_opt, resolve_instrument_id,
    IbkrCsvParser, SectionHeader,
};

pub fn parse_position_row(
    parser: &IbkrCsvParser,
    section: &str,
    headers: &HashMap<String, SectionHeader>,
    row: &[String],
    statement_end: Option<NaiveDate>,
    fallback_statement_date: Option<NaiveDate>,
    positions: &mut Vec<Value>,
    instruments: &mut Vec<Value>,
    instrument_key_to_id: &mut HashMap<String, String>,
) -> Result<bool> {
    if section != "Open Positions" {
        return Ok(false);
    }

    let header = headers
        .get(section)
        .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

    let row_data = PositionRowData::from_row(header, row);
    let Some(position) = map_position_from_row(
        parser,
        &row_data,
        statement_end,
        fallback_statement_date,
        instruments,
        instrument_key_to_id,
    ) else {
        return Ok(true);
    };

    positions.push(position);
    Ok(true)
}

pub fn reconcile_position_instrument_ids(
    positions: &mut [Value],
    account_id_savings: &str,
    instrument_id_remap: &HashMap<String, String>,
) {
    for position in positions.iter_mut() {
        let Some(old_instrument_id) = position.get("instrument_id").and_then(|v| v.as_str()) else {
            continue;
        };

        let corrected_id = resolve_instrument_id(old_instrument_id, instrument_id_remap);
        if corrected_id == old_instrument_id {
            continue;
        }

        if let Some(instrument_id_field) = position.get_mut("instrument_id") {
            *instrument_id_field = Value::String(corrected_id.clone());
        }

        if let Some(as_of) = position.get("as_of_date").and_then(|v| v.as_str()) {
            let key = format!("{}|{}|{}", account_id_savings, as_of, corrected_id);
            let hash = make_hash_id(&key);
            if let Some(position_id_field) = position.get_mut("position_id") {
                *position_id_field = Value::String(format!("IBKRPOS-{}", &hash[..12]));
            }
        }
    }
}

#[derive(Debug, Clone)]
struct PositionRowData {
    asset_category: String,
    currency: String,
    symbol: String,
    quantity: Option<f64>,
    cost_price: Option<f64>,
    cost_basis: Option<f64>,
    close_price: Option<f64>,
    market_value: Option<f64>,
    unrealized_pnl: Option<f64>,
}

impl PositionRowData {
    fn from_row(header: &SectionHeader, row: &[String]) -> Self {
        Self {
            asset_category: header
                .get(row, "Asset Category")
                .unwrap_or("")
                .trim()
                .to_string(),
            currency: header.get(row, "Currency").unwrap_or("").trim().to_string(),
            symbol: header.get(row, "Symbol").unwrap_or("").trim().to_string(),
            quantity: parse_f64_opt(header.get(row, "Quantity").unwrap_or("")),
            cost_price: parse_f64_opt(header.get(row, "Cost Price").unwrap_or("")),
            cost_basis: parse_f64_opt(header.get(row, "Cost Basis").unwrap_or("")),
            close_price: parse_f64_opt(header.get(row, "Close Price").unwrap_or("")),
            market_value: parse_f64_opt(header.get(row, "Value").unwrap_or("")),
            unrealized_pnl: parse_f64_opt(header.get(row, "Unrealized P/L").unwrap_or("")),
        }
    }
}

fn map_position_from_row(
    parser: &IbkrCsvParser,
    row: &PositionRowData,
    statement_end: Option<NaiveDate>,
    fallback_statement_date: Option<NaiveDate>,
    instruments: &mut Vec<Value>,
    instrument_key_to_id: &mut HashMap<String, String>,
) -> Option<Value> {
    let quantity = field_quantity(row)?;
    let as_of_date = field_as_of_date(statement_end, fallback_statement_date);
    let instrument_id = field_instrument_id(row, instruments, instrument_key_to_id);
    let position_id = field_position_id(parser, &as_of_date, &instrument_id);

    Some(build_position(
        &PositionInput {
            position_id,
            source: field_source(),
            as_of_date: as_of_date.format("%Y-%m-%d").to_string(),
            account_id: field_account_id(parser),
            instrument_id,
            quantity: Some(quantity),
            currency: field_currency(row),
            cost_price: field_cost_price(row),
            cost_basis: field_cost_basis(row),
            close_price: field_close_price(row),
            market_value: field_market_value(row),
        },
        field_unrealized_pnl(row),
    ))
}

fn field_source() -> String {
    "IBKR".to_string()
}

fn field_account_id(parser: &IbkrCsvParser) -> String {
    parser.account_id_savings.clone()
}

fn field_as_of_date(
    statement_end: Option<NaiveDate>,
    fallback_statement_date: Option<NaiveDate>,
) -> NaiveDate {
    statement_end
        .or(fallback_statement_date)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
}

fn field_position_id(parser: &IbkrCsvParser, as_of_date: &NaiveDate, instrument_id: &str) -> String {
    let key = format!(
        "{}|{}|{}",
        parser.account_id_savings,
        as_of_date.format("%Y-%m-%d"),
        instrument_id
    );
    let hash = make_hash_id(&key);
    format!("IBKRPOS-{}", &hash[..12])
}

fn field_instrument_id(
    row: &PositionRowData,
    instruments: &mut Vec<Value>,
    instrument_key_to_id: &mut HashMap<String, String>,
) -> String {
    let lookup_key = format!("{}|{}", row.asset_category, row.symbol);

    if let Some(existing_id) = instrument_key_to_id.get(&lookup_key) {
        return existing_id.clone();
    }

    let fallback_id = build_instrument_id("", "", &row.asset_category, &row.symbol);
    let placeholder = json!({
        "instrument_id": fallback_id,
        "source": "IBKR",
        "asset_category": null_if_empty_string(&row.asset_category),
        "description": null,
        "security_id": null,
        "type": null
    });

    instruments.push(placeholder);
    instrument_key_to_id.insert(lookup_key, fallback_id.clone());
    fallback_id
}

fn field_quantity(row: &PositionRowData) -> Option<f64> {
    row.quantity
}

fn field_currency(row: &PositionRowData) -> Option<String> {
    null_if_empty_string(&row.currency)
}

fn field_cost_price(row: &PositionRowData) -> Option<f64> {
    row.cost_price
}

fn field_cost_basis(row: &PositionRowData) -> Option<f64> {
    row.cost_basis
}

fn field_close_price(row: &PositionRowData) -> Option<f64> {
    row.close_price
}

fn field_market_value(row: &PositionRowData) -> Option<f64> {
    row.market_value
}

fn field_unrealized_pnl(row: &PositionRowData) -> Option<f64> {
    row.unrealized_pnl
}
