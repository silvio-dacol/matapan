use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashSet;

/// Merges new instruments into an existing database template with duplicate detection.
/// Instruments are considered duplicates if they have the same `instrument_id`.
///
/// # Arguments
/// * `template` - The existing database.json as a Value
/// * `new_instruments` - Vector of new instrument objects to merge
///
/// # Returns
/// * `Result<(Value, crate::MergeStats)>` - The merged database and merge statistics
pub fn merge_instruments_with_deduplication(
    mut template: Value,
    new_instruments: Vec<Value>,
) -> Result<(Value, crate::MergeStats)> {
    let arr = template
        .get_mut("instruments")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("database.json missing 'instruments' array"))?;

    for existing in arr.iter_mut() {
        *existing = normalize_instrument(existing)?;
    }

    let mut existing_ids: HashSet<String> = arr
        .iter()
        .filter_map(|inst| {
            inst.get("instrument_id")
                .and_then(|id| id.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let mut stats = crate::MergeStats {
        added: 0,
        skipped: 0,
        total: new_instruments.len(),
    };

    for instrument in new_instruments {
        let normalized_instrument = normalize_instrument(&instrument)?;

        let instrument_id = normalized_instrument
            .get("instrument_id")
            .and_then(|id| id.as_str())
            .ok_or_else(|| anyhow!("Instrument missing 'instrument_id' field"))?
            .to_string();

        if existing_ids.contains(&instrument_id) {
            stats.skipped += 1;
        } else {
            arr.push(normalized_instrument);
            existing_ids.insert(instrument_id);
            stats.added += 1;
        }
    }

    Ok((template, stats))
}

/// Finds and returns a list of instrument IDs that already exist in the database.
/// This can be useful for reporting which instruments were duplicates.
///
/// # Arguments
/// * `database` - The database.json as a Value
/// * `instrument_ids` - A slice of instrument IDs to check
///
/// # Returns
/// * `Result<Vec<String>>` - List of instrument IDs already present in the database
pub fn find_duplicate_instrument_ids(
    database: &Value,
    instrument_ids: &[String],
) -> Result<Vec<String>> {
    let arr = database
        .get("instruments")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("database.json missing 'instruments' array"))?;

    let existing_ids: HashSet<String> = arr
        .iter()
        .filter_map(|inst| {
            inst.get("instrument_id")
                .and_then(|id| id.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let duplicates: Vec<String> = instrument_ids
        .iter()
        .filter(|id| existing_ids.contains(*id))
        .cloned()
        .collect();

    Ok(duplicates)
}

fn normalize_instrument(instrument: &Value) -> Result<Value> {
    let instrument_id = instrument
        .get("instrument_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Instrument missing 'instrument_id' field"))?;

    Ok(serde_json::json!({
        "instrument_id": instrument_id,
        "source": field_or_null(instrument, "source"),
        "asset_category": field_or_null(instrument, "asset_category"),
        "description": field_or_null(instrument, "description"),
        "security_id": field_or_null(instrument, "security_id"),
        "type": field_or_null(instrument, "type")
    }))
}

fn field_or_null(instrument: &Value, field: &str) -> Value {
    instrument.get(field).cloned().unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_merge_with_no_duplicates() {
        let database = json!({
            "instruments": [
                {"instrument_id": "ISIN_A", "description": "A"}
            ]
        });

        let new_instruments = vec![
            json!({"instrument_id": "ISIN_B", "description": "B"}),
            json!({"instrument_id": "ISIN_C", "description": "C"}),
        ];

        let (merged, stats) =
            merge_instruments_with_deduplication(database, new_instruments).unwrap();

        assert_eq!(stats.added, 2);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.total, 2);

        let instruments = merged.get("instruments").unwrap().as_array().unwrap();
        assert_eq!(instruments.len(), 3);
    }

    #[test]
    fn test_merge_with_duplicates() {
        let database = json!({
            "instruments": [
                {"instrument_id": "ISIN_A", "description": "A"},
                {"instrument_id": "ISIN_B", "description": "B"}
            ]
        });

        let new_instruments = vec![
            json!({"instrument_id": "ISIN_B", "description": "B"}),
            json!({"instrument_id": "ISIN_C", "description": "C"}),
        ];

        let (merged, stats) =
            merge_instruments_with_deduplication(database, new_instruments).unwrap();

        assert_eq!(stats.added, 1);
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.total, 2);
        assert!(stats.has_duplicates());

        let instruments = merged.get("instruments").unwrap().as_array().unwrap();
        assert_eq!(instruments.len(), 3);
    }

    #[test]
    fn test_find_duplicate_instrument_ids() {
        let database = json!({
            "instruments": [
                {"instrument_id": "ISIN_A", "description": "A"},
                {"instrument_id": "ISIN_B", "description": "B"}
            ]
        });

        let check_ids = vec![
            "ISIN_A".to_string(),
            "ISIN_C".to_string(),
            "ISIN_B".to_string(),
        ];

        let duplicates = find_duplicate_instrument_ids(&database, &check_ids).unwrap();

        assert_eq!(duplicates.len(), 2);
        assert!(duplicates.contains(&"ISIN_A".to_string()));
        assert!(duplicates.contains(&"ISIN_B".to_string()));
        assert!(!duplicates.contains(&"ISIN_C".to_string()));
    }

    #[test]
    fn test_merge_normalizes_schema() {
        let database = json!({
            "instruments": [
                {
                    "instrument_id": "IBKR_IE00B5BMR087",
                    "source": "IBKR",
                    "asset_category": "Stocks",
                    "symbol": "CSPX, SXR8",
                    "description": "ISHARES CORE S&P 500",
                    "conid": "75776072",
                    "security_id": "IE00B5BMR087",
                    "listing_exchange": "IBIS2",
                    "type": "ETF",
                    "underlying": "SXR8",
                    "multiplier": 1.0
                }
            ]
        });

        let new_instruments = vec![];
        let (merged, _stats) =
            merge_instruments_with_deduplication(database, new_instruments).unwrap();

        let inst = merged
            .get("instruments")
            .and_then(|v| v.as_array())
            .and_then(|v| v.first())
            .unwrap();

        assert_eq!(inst.get("instrument_id").and_then(|v| v.as_str()), Some("IBKR_IE00B5BMR087"));
        assert_eq!(inst.get("source").and_then(|v| v.as_str()), Some("IBKR"));
        assert_eq!(inst.get("asset_category").and_then(|v| v.as_str()), Some("Stocks"));
        assert_eq!(inst.get("description").and_then(|v| v.as_str()), Some("ISHARES CORE S&P 500"));
        assert_eq!(inst.get("security_id").and_then(|v| v.as_str()), Some("IE00B5BMR087"));
        assert_eq!(inst.get("type").and_then(|v| v.as_str()), Some("ETF"));
        assert!(inst.get("symbol").is_none());
        assert!(inst.get("conid").is_none());
        assert!(inst.get("listing_exchange").is_none());
        assert!(inst.get("underlying").is_none());
        assert!(inst.get("multiplier").is_none());
    }

    #[test]
    fn test_merge_skips_duplicates_within_same_batch() {
        let database = json!({ "instruments": [] });

        let new_instruments = vec![
            json!({"instrument_id": "ISIN_X", "description": "A", "source": "IBKR"}),
            json!({"instrument_id": "ISIN_X", "description": "B", "source": "IBKR"}),
        ];

        let (merged, stats) =
            merge_instruments_with_deduplication(database, new_instruments).unwrap();

        assert_eq!(stats.added, 1);
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.total, 2);
        let instruments = merged.get("instruments").unwrap().as_array().unwrap();
        assert_eq!(instruments.len(), 1);
    }
}