use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::{
    build_instrument_id, merge_instrument_entries, null_if_empty_string, primary_symbol,
    resolve_instrument_id, split_symbol_aliases, SectionHeader,
};

pub fn parse_instrument_row(
    section: &str,
    headers: &HashMap<String, SectionHeader>,
    row: &[String],
    instruments: &mut Vec<Value>,
    instrument_key_to_id: &mut HashMap<String, String>,
    instrument_id_remap: &mut HashMap<String, String>,
) -> Result<bool> {
    if section != "Financial Instrument Information" {
        return Ok(false);
    }

    let header = headers
        .get(section)
        .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

    let row_data = InstrumentRowData::from_row(header, row);
    let Some(instrument) = map_instrument_from_row(&row_data) else {
        return Ok(true);
    };

    let aliases = field_symbol_aliases(&row_data);
    let existing_id = find_existing_instrument_id(
        &aliases,
        field_asset_category_key(&row_data),
        field_symbol_key(&row_data),
        instrument_key_to_id,
        instrument_id_remap,
    );

    let parsed_instrument_id = field_instrument_id(&row_data);
    let final_instrument_id = upsert_instrument(
        instrument,
        existing_id,
        &parsed_instrument_id,
        instruments,
        instrument_id_remap,
    );

    index_instrument_aliases(
        &aliases,
        field_asset_category_key(&row_data),
        field_symbol_key(&row_data),
        &final_instrument_id,
        instrument_key_to_id,
    );

    Ok(true)
}

#[derive(Debug, Clone)]
struct InstrumentRowData {
    asset_category: String,
    symbol: String,
    description: String,
    conid: String,
    security_id: String,
    instrument_type: String,
}

impl InstrumentRowData {
    fn from_row(header: &SectionHeader, row: &[String]) -> Self {
        Self {
            asset_category: header
                .get(row, "Asset Category")
                .unwrap_or("")
                .trim()
                .to_string(),
            symbol: header.get(row, "Symbol").unwrap_or("").trim().to_string(),
            description: header
                .get(row, "Description")
                .unwrap_or("")
                .trim()
                .to_string(),
            conid: header.get(row, "Conid").unwrap_or("").trim().to_string(),
            security_id: header
                .get(row, "Security ID")
                .unwrap_or("")
                .trim()
                .to_string(),
            instrument_type: header.get(row, "Type").unwrap_or("").trim().to_string(),
        }
    }
}

fn map_instrument_from_row(row: &InstrumentRowData) -> Option<Value> {
    if field_is_empty_row(row) {
        return None;
    }

    Some(json!({
        "instrument_id": field_instrument_id(row),
        "source": field_source(),
        "asset_category": field_asset_category(row),
        "description": field_description(row),
        "security_id": field_security_id(row),
        "type": field_type(row)
    }))
}

fn field_is_empty_row(row: &InstrumentRowData) -> bool {
    row.symbol.is_empty() && row.conid.is_empty() && row.security_id.is_empty()
}

fn field_instrument_id(row: &InstrumentRowData) -> String {
    build_instrument_id(
        field_conid_key(row),
        field_security_id_key(row),
        field_asset_category_key(row),
        &field_primary_symbol(row),
    )
}

fn field_source() -> &'static str {
    "IBKR"
}

fn field_asset_category(row: &InstrumentRowData) -> Option<String> {
    null_if_empty_string(&row.asset_category)
}

fn field_description(row: &InstrumentRowData) -> Option<String> {
    null_if_empty_string(&row.description)
}

fn field_security_id(row: &InstrumentRowData) -> Option<String> {
    null_if_empty_string(&row.security_id)
}

fn field_type(row: &InstrumentRowData) -> Option<String> {
    null_if_empty_string(&row.instrument_type)
}

fn field_asset_category_key(row: &InstrumentRowData) -> &str {
    row.asset_category.as_str()
}

fn field_symbol_key(row: &InstrumentRowData) -> &str {
    row.symbol.as_str()
}

fn field_conid_key(row: &InstrumentRowData) -> &str {
    row.conid.as_str()
}

fn field_security_id_key(row: &InstrumentRowData) -> &str {
    row.security_id.as_str()
}

fn field_primary_symbol(row: &InstrumentRowData) -> String {
    primary_symbol(&row.symbol)
}

fn field_symbol_aliases(row: &InstrumentRowData) -> Vec<String> {
    split_symbol_aliases(&row.symbol)
}

fn find_existing_instrument_id(
    aliases: &[String],
    asset_category: &str,
    symbol: &str,
    instrument_key_to_id: &HashMap<String, String>,
    instrument_id_remap: &HashMap<String, String>,
) -> Option<String> {
    for alias in aliases {
        let key = format!("{}|{}", asset_category, alias);
        if let Some(id) = instrument_key_to_id.get(&key) {
            return Some(resolve_instrument_id(id, instrument_id_remap));
        }
    }

    let full_symbol_key = format!("{}|{}", asset_category, symbol);
    instrument_key_to_id
        .get(&full_symbol_key)
        .map(|id| resolve_instrument_id(id, instrument_id_remap))
}

fn upsert_instrument(
    instrument: Value,
    existing_id: Option<String>,
    parsed_instrument_id: &str,
    instruments: &mut Vec<Value>,
    instrument_id_remap: &mut HashMap<String, String>,
) -> String {
    if let Some(existing_id) = existing_id {
        if let Some(index) = instruments.iter().position(|item| {
            item.get("instrument_id")
                .and_then(|v| v.as_str())
                .map(|id| id == existing_id)
                .unwrap_or(false)
        }) {
            let merged = merge_instrument_entries(&instruments[index], &instrument);
            let merged_asset_category = merged
                .get("asset_category")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let merged_symbol = merged.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
            let merged_conid = merged.get("conid").and_then(|v| v.as_str()).unwrap_or("");
            let merged_security_id = merged
                .get("security_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let new_instrument_id = build_instrument_id(
                merged_conid,
                merged_security_id,
                merged_asset_category,
                &primary_symbol(merged_symbol),
            );

            let mut merged_with_id = merged;
            if let Some(id_field) = merged_with_id.get_mut("instrument_id") {
                *id_field = Value::String(new_instrument_id.clone());
            }

            instruments[index] = merged_with_id;

            if new_instrument_id != existing_id {
                instrument_id_remap.insert(existing_id.clone(), new_instrument_id.clone());
            }

            return new_instrument_id;
        }

        instruments.push(instrument);
        return parsed_instrument_id.to_string();
    }

    if let Some(index) = instruments.iter().position(|item| {
        item.get("instrument_id")
            .and_then(|v| v.as_str())
            .map(|id| id == parsed_instrument_id)
            .unwrap_or(false)
    }) {
        instruments[index] = merge_instrument_entries(&instruments[index], &instrument);
        return parsed_instrument_id.to_string();
    }

    instruments.push(instrument);
    parsed_instrument_id.to_string()
}

fn index_instrument_aliases(
    aliases: &[String],
    asset_category: &str,
    symbol: &str,
    instrument_id: &str,
    instrument_key_to_id: &mut HashMap<String, String>,
) {
    for alias in aliases {
        let key = format!("{}|{}", asset_category, alias);
        instrument_key_to_id.insert(key, instrument_id.to_string());
    }

    let full_symbol_key = format!("{}|{}", asset_category, symbol);
    instrument_key_to_id.insert(full_symbol_key, instrument_id.to_string());
}
