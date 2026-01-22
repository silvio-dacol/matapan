use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Merges new transactions into an existing database template with duplicate detection.
/// Transactions are considered duplicates if they have the same `txn_id`.
///
/// # Arguments
/// * `template` - The existing database.json as a Value
/// * `new_txns` - Vector of new transaction objects to merge
///
/// # Returns
/// * `Result<(Value, MergeStats)>` - The merged database and statistics about the merge
///
/// # Example
/// ```no_run
/// use utils::merge_transactions_with_deduplication;
/// use serde_json::json;
///
/// let database = json!({"transactions": []});
/// let new_txns = vec![json!({"txn_id": "ABC123", "amount": 100.0})];
/// let (merged, stats) = merge_transactions_with_deduplication(database, new_txns).unwrap();
/// println!("Added: {}, Skipped: {}", stats.added, stats.skipped);
/// ```
pub fn merge_transactions_with_deduplication(
    mut template: Value,
    new_txns: Vec<Value>,
) -> Result<(Value, MergeStats)> {
    let arr = template
        .get_mut("transactions")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("database.json missing 'transactions' array"))?;

    // Build a set of existing transaction IDs
    let existing_ids: HashSet<String> = arr
        .iter()
        .filter_map(|txn| {
            txn.get("txn_id")
                .and_then(|id| id.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let mut stats = MergeStats {
        added: 0,
        skipped: 0,
        total: new_txns.len(),
    };

    // Only add transactions that don't already exist
    for txn in new_txns {
        let txn_id = txn
            .get("txn_id")
            .and_then(|id| id.as_str())
            .ok_or_else(|| anyhow!("Transaction missing 'txn_id' field"))?;

        if existing_ids.contains(txn_id) {
            stats.skipped += 1;
        } else {
            arr.push(txn);
            stats.added += 1;
        }
    }

    Ok((template, stats))
}

/// Statistics about a transaction merge operation
#[derive(Debug, Clone)]
pub struct MergeStats {
    pub added: usize,
    pub skipped: usize,
    pub total: usize,
}

impl MergeStats {
    pub fn has_duplicates(&self) -> bool {
        self.skipped > 0
    }
}

/// Deduplicate transactions in-place based on a strict signature of key fields.
/// Keeps the first occurrence and removes subsequent ones, preserving order.
///
/// Signature includes: `date`, `amount`, `currency`, `from_account_id`, `to_account_id`, `type`.
/// This is conservative and avoids collapsing distinct records that differ in `type`.
///
/// Returns the count of removed transactions.
pub fn dedup_transactions_by_signature(database: &mut Value) -> Result<usize> {
    let arr = database
        .get_mut("transactions")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("database.json missing 'transactions' array"))?;

    let mut seen: HashSet<String> = HashSet::new();
    let mut deduped: Vec<Value> = Vec::with_capacity(arr.len());
    let mut removed = 0usize;

    for txn in arr.drain(..) {
        if let Some(sig) = build_signature(&txn) {
            if seen.insert(sig) {
                deduped.push(txn);
            } else {
                removed += 1;
            }
        } else {
            // If signature cannot be built, keep the transaction (no risky removal)
            deduped.push(txn);
        }
    }

    *arr = deduped;
    Ok(removed)
}

fn build_signature(txn: &Value) -> Option<String> {
    let obj = txn.as_object()?;
    let date = obj.get("date")?.as_str()?;
    let amount = obj.get("amount")?.to_string();
    let currency = obj.get("currency")?.as_str()?;
    let from = obj.get("from_account_id")?.as_str()?;
    let to = obj.get("to_account_id")?.as_str()?;
    let typ = obj.get("type")?.as_str()?;
    Some(format!(
        "{}|{}|{}|{}|{}|{}",
        date, amount, currency, from, to, typ
    ))
}

/// Mark duplicates in-place based on the same strict signature used by
/// `dedup_transactions_by_signature()`. Keeps all transactions, but annotates
/// later duplicates with `{"duplicate": true, "duplicate_of_txn_id": "..."}` when possible.
/// Returns the count of marked duplicates.
pub fn mark_duplicates_by_signature(database: &mut Value) -> Result<usize> {
    let arr = database
        .get_mut("transactions")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("database.json missing 'transactions' array"))?;

    let mut first_seen: HashMap<String, Option<String>> = HashMap::new();
    let mut marked = 0usize;

    for txn in arr.iter_mut() {
        let Some(sig) = build_signature(txn) else {
            continue;
        };

        if !first_seen.contains_key(&sig) {
            // Record first occurrence and its txn_id (if present)
            let first_id = txn
                .get("txn_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            first_seen.insert(sig, first_id);
        } else {
            // Subsequent occurrence: mark as duplicate
            if let Some(obj) = txn.as_object_mut() {
                obj.insert("duplicate".to_string(), Value::Bool(true));
                if let Some(Some(id)) = first_seen.get(&sig) {
                    obj.insert("duplicate_of_txn_id".to_string(), Value::String(id.clone()));
                }
                marked += 1;
            }
        }
    }

    Ok(marked)
}

/// Finds and returns a list of transaction IDs that already exist in the database.
/// This can be useful for reporting which transactions were duplicates.
///
/// # Arguments
/// * `database` - The database.json as a Value
/// * `txn_ids` - A slice of transaction IDs to check
///
/// # Returns
/// * `Result<Vec<String>>` - List of txn_ids that already exist in the database
pub fn find_duplicate_txn_ids(database: &Value, txn_ids: &[String]) -> Result<Vec<String>> {
    let arr = database
        .get("transactions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("database.json missing 'transactions' array"))?;

    let existing_ids: HashSet<String> = arr
        .iter()
        .filter_map(|txn| {
            txn.get("txn_id")
                .and_then(|id| id.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let duplicates: Vec<String> = txn_ids
        .iter()
        .filter(|id| existing_ids.contains(*id))
        .cloned()
        .collect();

    Ok(duplicates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_merge_with_no_duplicates() {
        let database = json!({
            "transactions": [
                {"txn_id": "TXN001", "amount": 100.0}
            ]
        });

        let new_txns = vec![
            json!({"txn_id": "TXN002", "amount": 200.0}),
            json!({"txn_id": "TXN003", "amount": 300.0}),
        ];

        let (merged, stats) = merge_transactions_with_deduplication(database, new_txns).unwrap();

        assert_eq!(stats.added, 2);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.total, 2);

        let txns = merged.get("transactions").unwrap().as_array().unwrap();
        assert_eq!(txns.len(), 3);
    }

    #[test]
    fn test_merge_with_duplicates() {
        let database = json!({
            "transactions": [
                {"txn_id": "TXN001", "amount": 100.0},
                {"txn_id": "TXN002", "amount": 200.0}
            ]
        });

        let new_txns = vec![
            json!({"txn_id": "TXN002", "amount": 200.0}), // duplicate
            json!({"txn_id": "TXN003", "amount": 300.0}), // new
        ];

        let (merged, stats) = merge_transactions_with_deduplication(database, new_txns).unwrap();

        assert_eq!(stats.added, 1);
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.total, 2);
        assert!(stats.has_duplicates());

        let txns = merged.get("transactions").unwrap().as_array().unwrap();
        assert_eq!(txns.len(), 3); // Should still be 3, not 4
    }

    #[test]
    fn test_find_duplicate_txn_ids() {
        let database = json!({
            "transactions": [
                {"txn_id": "TXN001", "amount": 100.0},
                {"txn_id": "TXN002", "amount": 200.0}
            ]
        });

        let check_ids = vec![
            "TXN001".to_string(),
            "TXN003".to_string(),
            "TXN002".to_string(),
        ];

        let duplicates = find_duplicate_txn_ids(&database, &check_ids).unwrap();

        assert_eq!(duplicates.len(), 2);
        assert!(duplicates.contains(&"TXN001".to_string()));
        assert!(duplicates.contains(&"TXN002".to_string()));
        assert!(!duplicates.contains(&"TXN003".to_string()));
    }

    #[test]
    fn test_dedup_transactions_by_signature() {
        let mut database = json!({
            "transactions": [
                {
                    "date": "2025-01-01",
                    "from_account_id": "A",
                    "to_account_id": "B",
                    "type": "internal_transfer",
                    "category": "uncategorized",
                    "amount": 100.0,
                    "currency": "SEK",
                    "description": "A->B",
                    "txn_id": "X1"
                },
                {
                    "date": "2025-01-01",
                    "from_account_id": "A",
                    "to_account_id": "B",
                    "type": "internal_transfer",
                    "category": "uncategorized",
                    "amount": 100.0,
                    "currency": "SEK",
                    "description": "Duplicate",
                    "txn_id": "X2"
                },
                {
                    "date": "2025-01-01",
                    "from_account_id": "A",
                    "to_account_id": "B",
                    "type": "income",
                    "category": "uncategorized",
                    "amount": 100.0,
                    "currency": "SEK",
                    "description": "Same movement but different type",
                    "txn_id": "X3"
                }
            ]
        });

        let removed = dedup_transactions_by_signature(&mut database).unwrap();
        assert_eq!(removed, 1);

        let arr = database.get("transactions").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 2);
        // Ensure first occurrence kept and different type retained
        assert_eq!(arr[0].get("txn_id").unwrap().as_str().unwrap(), "X1");
        assert_eq!(arr[1].get("txn_id").unwrap().as_str().unwrap(), "X3");
    }

    #[test]
    fn test_mark_duplicates_by_signature() {
        let mut database = json!({
            "transactions": [
                {
                    "date": "2025-01-01",
                    "from_account_id": "A",
                    "to_account_id": "B",
                    "type": "internal_transfer",
                    "category": "uncategorized",
                    "amount": 100.0,
                    "currency": "SEK",
                    "description": "A->B",
                    "txn_id": "X1"
                },
                {
                    "date": "2025-01-01",
                    "from_account_id": "A",
                    "to_account_id": "B",
                    "type": "internal_transfer",
                    "category": "uncategorized",
                    "amount": 100.0,
                    "currency": "SEK",
                    "description": "Duplicate",
                    "txn_id": "X2"
                }
            ]
        });

        let marked = mark_duplicates_by_signature(&mut database).unwrap();
        assert_eq!(marked, 1);

        let arr = database.get("transactions").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[1].get("duplicate").unwrap().as_bool().unwrap(), true);
        assert_eq!(
            arr[1].get("duplicate_of_txn_id").unwrap().as_str().unwrap(),
            "X1"
        );
    }
}
