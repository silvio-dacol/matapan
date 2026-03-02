//! Shared monetary rounding helpers.

use serde_json::Value;

const MONEY_DECIMALS: i32 = 2;
const HALF_EPSILON: f64 = 1e-9;

/// Rounds a monetary value to 2 decimals using half-down semantics.
///
/// Tie behavior at exactly x.xx5 rounds toward zero.
pub fn round_money(value: f64) -> f64 {
    round_half_down(value, MONEY_DECIMALS)
}

pub fn round_money_option(value: Option<f64>) -> Option<f64> {
    value.map(round_money)
}

pub fn round_money_value_field(value: &mut Value) -> bool {
    let Some(number) = value.as_f64() else {
        return false;
    };

    let rounded = round_money(number);
    if rounded == number {
        return false;
    }

    *value = Value::from(rounded);
    true
}

fn round_half_down(value: f64, decimals: i32) -> f64 {
    let factor = 10f64.powi(decimals);
    let scaled = value * factor;
    let sign = scaled.signum();
    let abs_scaled = scaled.abs();
    let floor = abs_scaled.floor();
    let frac = abs_scaled - floor;

    let rounded_abs = if frac > 0.5 + HALF_EPSILON {
        floor + 1.0
    } else {
        floor
    };

    let out = rounded_abs * sign / factor;
    if out == -0.0 {
        0.0
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_money_uses_half_down_for_positive_values() {
        assert_eq!(round_money(12.345), 12.34);
        assert_eq!(round_money(12.346), 12.35);
    }

    #[test]
    fn round_money_uses_half_down_for_negative_values() {
        assert_eq!(round_money(-12.345), -12.34);
        assert_eq!(round_money(-12.346), -12.35);
    }
}
