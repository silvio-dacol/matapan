#![allow(dead_code)]

//! Placeholder mapping hooks for future Alipay investment instrument parsing.

use serde_json::Value;
use utils::{build_instrument, InstrumentInput};

pub fn create_instruments() -> Vec<Value> {
    Vec::new()
}

pub fn map_instrument_from_row() -> Option<Value> {
    let instrument_id = field_instrument_id()?;

    Some(build_instrument(&InstrumentInput {
        instrument_id,
        source: field_source(),
        asset_category: field_asset_category(),
        description: field_description(),
        security_id: field_security_id(),
        instrument_type: field_type(),
    }))
}

fn field_instrument_id() -> Option<String> {
    None
}

fn field_source() -> Option<String> {
    None
}

fn field_asset_category() -> Option<String> {
    None
}

fn field_description() -> Option<String> {
    None
}

fn field_security_id() -> Option<String> {
    None
}

fn field_type() -> Option<String> {
    None
}
