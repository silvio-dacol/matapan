use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct PositionInput {
    pub position_id: String,
    pub source: String,
    pub as_of_date: String,
    pub account_id: String,
    pub instrument_id: String,
    pub quantity: Option<f64>,
    pub currency: Option<String>,
    pub cost_price: Option<f64>,
    pub cost_basis: Option<f64>,
    pub close_price: Option<f64>,
    pub market_value: Option<f64>,
}

pub fn split_unrealized_pnl(unrealized_pnl: Option<f64>) -> (Option<f64>, Option<f64>) {
    if let Some(pnl) = unrealized_pnl {
        if pnl > 0.0 {
            (Some(pnl), Some(0.0))
        } else if pnl < 0.0 {
            (Some(0.0), Some(-pnl))
        } else {
            (Some(0.0), Some(0.0))
        }
    } else {
        (None, None)
    }
}

pub fn build_position(input: &PositionInput, unrealized_pnl: Option<f64>) -> Value {
    let (unrealized_profit, unrealized_loss) = split_unrealized_pnl(unrealized_pnl);

    serde_json::json!({
        "position_id": input.position_id,
        "source": input.source,
        "as_of_date": input.as_of_date,
        "account_id": input.account_id,
        "instrument_id": input.instrument_id,
        "quantity": input.quantity,
        "currency": input.currency,
        "cost_price": input.cost_price,
        "cost_basis": input.cost_basis,
        "close_price": input.close_price,
        "market_value": input.market_value,
        "unrealized_profit": unrealized_profit,
        "unrealized_loss": unrealized_loss
    })
}

pub fn normalize_position_pnl_fields(position: &mut Value) -> bool {
    let Some(obj) = position.as_object_mut() else {
        return false;
    };

    let legacy_pnl = obj.get("unrealized_pnl").and_then(|v| v.as_f64());
    let current_profit = obj.get("unrealized_profit").and_then(|v| v.as_f64());
    let current_loss = obj.get("unrealized_loss").and_then(|v| v.as_f64());

    let (profit, loss) = if legacy_pnl.is_some() {
        split_unrealized_pnl(legacy_pnl)
    } else if current_profit.is_some() || current_loss.is_some() {
        (
            Some(current_profit.unwrap_or(0.0).abs()),
            Some(current_loss.unwrap_or(0.0).abs()),
        )
    } else {
        return false;
    };

    obj.insert(
        "unrealized_profit".to_string(),
        profit.map_or(Value::Null, Value::from),
    );
    obj.insert(
        "unrealized_loss".to_string(),
        loss.map_or(Value::Null, Value::from),
    );

    obj.remove("unrealized_pnl").is_some()
        || legacy_pnl.is_some()
        || current_profit.is_some()
        || current_loss.is_some()
}

pub fn normalize_positions_pnl_fields(database: &mut Value) -> Result<usize> {
    let arr = database
        .get_mut("positions")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("database.json missing 'positions' array"))?;

    let mut normalized = 0usize;
    for pos in arr.iter_mut() {
        if normalize_position_pnl_fields(pos) {
            normalized += 1;
        }
    }

    Ok(normalized)
}

pub fn merge_positions_with_deduplication(
    mut template: Value,
    mut new_positions: Vec<Value>,
) -> Result<(Value, crate::MergeStats)> {
    let arr = template
        .get_mut("positions")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("database.json missing 'positions' array"))?;

    for pos in arr.iter_mut() {
        normalize_position_pnl_fields(pos);
    }
    for pos in new_positions.iter_mut() {
        normalize_position_pnl_fields(pos);
    }

    let existing: HashSet<String> = arr
        .iter()
        .filter_map(|v| {
            v.get("position_id")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let mut stats = crate::MergeStats {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_split_unrealized_pnl() {
        assert_eq!(split_unrealized_pnl(Some(10.0)), (Some(10.0), Some(0.0)));
        assert_eq!(split_unrealized_pnl(Some(-7.5)), (Some(0.0), Some(7.5)));
        assert_eq!(split_unrealized_pnl(Some(0.0)), (Some(0.0), Some(0.0)));
        assert_eq!(split_unrealized_pnl(None), (None, None));
    }

    #[test]
    fn test_build_position_standard_shape() {
        let pos = build_position(
            &PositionInput {
                position_id: "P1".to_string(),
                source: "TEST".to_string(),
                as_of_date: "2026-02-17".to_string(),
                account_id: "ACC".to_string(),
                instrument_id: "INST".to_string(),
                quantity: Some(2.0),
                currency: Some("EUR".to_string()),
                cost_price: Some(10.0),
                cost_basis: Some(20.0),
                close_price: Some(12.0),
                market_value: Some(24.0),
            },
            Some(4.0),
        );

        assert_eq!(
            pos.get("unrealized_profit").and_then(|v| v.as_f64()),
            Some(4.0)
        );
        assert_eq!(
            pos.get("unrealized_loss").and_then(|v| v.as_f64()),
            Some(0.0)
        );
    }

    #[test]
    fn test_normalize_position_from_legacy_pnl() {
        let mut pos = json!({
            "position_id": "P1",
            "unrealized_pnl": -47.47
        });

        let changed = normalize_position_pnl_fields(&mut pos);
        assert!(changed);
        assert!(pos.get("unrealized_pnl").is_none());
        assert_eq!(pos.get("unrealized_profit").and_then(|v| v.as_f64()), Some(0.0));
        assert_eq!(pos.get("unrealized_loss").and_then(|v| v.as_f64()), Some(47.47));
    }

    #[test]
    fn test_normalize_position_existing_fields_are_positive() {
        let mut pos = json!({
            "position_id": "P2",
            "unrealized_profit": -5.0,
            "unrealized_loss": -3.0
        });

        let changed = normalize_position_pnl_fields(&mut pos);
        assert!(changed);
        assert_eq!(pos.get("unrealized_profit").and_then(|v| v.as_f64()), Some(5.0));
        assert_eq!(pos.get("unrealized_loss").and_then(|v| v.as_f64()), Some(3.0));
    }

    #[test]
    fn test_normalize_positions_pnl_fields() {
        let mut db = json!({
            "positions": [
                {"position_id": "A", "unrealized_pnl": 3.0},
                {"position_id": "B", "unrealized_pnl": -2.0},
                {"position_id": "C"}
            ]
        });

        let normalized = normalize_positions_pnl_fields(&mut db).unwrap();
        assert_eq!(normalized, 2);
    }

    #[test]
    fn test_merge_positions_with_deduplication_normalizes_and_dedups() {
        let database = json!({
            "positions": [
                {"position_id": "P1", "unrealized_pnl": -2.0}
            ]
        });

        let new_positions = vec![
            json!({"position_id": "P1", "unrealized_pnl": 3.0}),
            json!({"position_id": "P2", "unrealized_pnl": 5.0}),
        ];

        let (merged, stats) = merge_positions_with_deduplication(database, new_positions).unwrap();
        assert_eq!(stats.added, 1);
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.total, 2);

        let arr = merged.get("positions").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert!(arr[0].get("unrealized_pnl").is_none());
        assert_eq!(
            arr[0].get("unrealized_loss").and_then(|v| v.as_f64()),
            Some(2.0)
        );
    }
}
