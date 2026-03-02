use serde_json::Value;
use utils::{build_instrument, InstrumentInput};

pub struct PortfolioRowData {
    pub isin: String,
    pub description: String,
}

pub fn map_instrument_from_row(row: &PortfolioRowData) -> Option<Value> {
    let instrument_id = field_instrument_id(row)?;

    Some(build_instrument(&InstrumentInput {
        instrument_id,
        source: field_source(),
        asset_category: field_asset_category(),
        description: field_description(row),
        security_id: field_security_id(row),
        instrument_type: field_type(),
    }))
}

fn field_instrument_id(row: &PortfolioRowData) -> Option<String> {
    if row.isin.trim().is_empty() {
        None
    } else {
        Some(format!("ISIN_{}", row.isin.trim()))
    }
}

fn field_source() -> Option<String> {
    Some("Intesa Sanpaolo".to_string())
}

fn field_asset_category() -> Option<String> {
    None
}

fn field_description(row: &PortfolioRowData) -> Option<String> {
    let value = row.description.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn field_security_id(row: &PortfolioRowData) -> Option<String> {
    let value = row.isin.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn field_type() -> Option<String> {
    None
}
