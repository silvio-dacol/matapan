#![allow(dead_code)]

//! Placeholder mapping hooks for future CarPay investment position parsing.

use serde_json::Value;
use utils::{build_position, PositionInput};

pub fn create_positions() -> Vec<Value> {
    Vec::new()
}

pub fn map_position_from_row() -> Option<Value> {
    let position_id = field_position_id()?;

    Some(build_position(
        &PositionInput {
            position_id,
            source: field_source().unwrap_or_default(),
            as_of_date: field_as_of_date().unwrap_or_default(),
            account_id: field_account_id().unwrap_or_default(),
            instrument_id: field_instrument_id().unwrap_or_default(),
            quantity: field_quantity(),
            currency: field_currency(),
            cost_price: field_cost_price(),
            cost_basis: field_cost_basis(),
            close_price: field_close_price(),
            market_value: field_market_value(),
        },
        field_unrealized_pnl(),
    ))
}

fn field_position_id() -> Option<String> {
    None
}

fn field_source() -> Option<String> {
    None
}

fn field_as_of_date() -> Option<String> {
    None
}

fn field_account_id() -> Option<String> {
    None
}

fn field_instrument_id() -> Option<String> {
    None
}

fn field_quantity() -> Option<f64> {
    None
}

fn field_currency() -> Option<String> {
    None
}

fn field_cost_price() -> Option<f64> {
    None
}

fn field_cost_basis() -> Option<f64> {
    None
}

fn field_close_price() -> Option<f64> {
    None
}

fn field_market_value() -> Option<f64> {
    None
}

fn field_unrealized_pnl() -> Option<f64> {
    None
}
