use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashSet;

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
}
