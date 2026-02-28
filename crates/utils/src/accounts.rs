//! Builds and merges normalized account entities in database.json.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct AccountInput {
    pub account_id: String,
    pub institution: String,
    pub country: Option<String>,
    pub iban: Option<String>,
    pub bic: Option<String>,
    pub is_active: bool,
}

pub fn build_account(input: &AccountInput) -> Value {
    json!({
        "account_id": input.account_id,
        "institution": input.institution,
        "country": input.country,
        "iban": input.iban,
        "bic": input.bic,
        "is_active": input.is_active
    })
}

/// Merges new accounts into an existing database template with duplicate detection.
/// Accounts are considered duplicates if they have the same `account_id`.
///
/// # Arguments
/// * `template` - The existing database.json as a Value
/// * `new_accounts` - Vector of new account objects to merge
///
/// # Returns
/// * `Result<(Value, MergeStats)>` - The merged database and statistics about the merge
///
/// # Example
/// ```no_run
/// use utils::merge_accounts_with_deduplication;
/// use serde_json::json;
///
/// let database = json!({"accounts": []});
/// let new_accounts = vec![json!({"account_id": "REVOLUT_CURRENT", "institution": "Revolut"})];
/// let (merged, stats) = merge_accounts_with_deduplication(database, new_accounts).unwrap();
/// println!("Added: {}, Skipped: {}", stats.added, stats.skipped);
/// ```
pub fn merge_accounts_with_deduplication(
    mut template: Value,
    new_accounts: Vec<Value>,
) -> Result<(Value, MergeStats)> {
    let arr = template
        .get_mut("accounts")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("database.json missing 'accounts' array"))?;

    // Build a set of existing account IDs
    let existing_ids: HashSet<String> = arr
        .iter()
        .filter_map(|acc| {
            acc.get("account_id")
                .and_then(|id| id.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let mut stats = MergeStats {
        added: 0,
        skipped: 0,
        total: new_accounts.len(),
    };

    // Only add accounts that don't already exist
    for account in new_accounts {
        let account_id = account
            .get("account_id")
            .and_then(|id| id.as_str())
            .ok_or_else(|| anyhow!("Account missing 'account_id' field"))?;

        if existing_ids.contains(account_id) {
            stats.skipped += 1;
        } else {
            arr.push(account);
            stats.added += 1;
        }
    }

    Ok((template, stats))
}

/// Statistics about an account merge operation
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

/// Finds and returns a list of account IDs that already exist in the database.
/// This can be useful for reporting which accounts were duplicates.
///
/// # Arguments
/// * `database` - The database.json as a Value
/// * `account_ids` - A slice of account IDs to check
///
/// # Returns
/// * `Result<Vec<String>>` - List of account_ids that already exist in the database
pub fn find_duplicate_account_ids(database: &Value, account_ids: &[String]) -> Result<Vec<String>> {
    let arr = database
        .get("accounts")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("database.json missing 'accounts' array"))?;

    let existing_ids: HashSet<String> = arr
        .iter()
        .filter_map(|acc| {
            acc.get("account_id")
                .and_then(|id| id.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let duplicates: Vec<String> = account_ids
        .iter()
        .filter(|id| existing_ids.contains(*id))
        .cloned()
        .collect();

    Ok(duplicates)
}

/// Creates standard system accounts that are used across all parsers.
/// These accounts represent external parties and are used when money flows
/// in/out of the user's accounts from/to unknown external sources.
///
/// Returns a vector of system account entries.
pub fn create_system_accounts() -> Vec<Value> {
    vec![
        build_account(&AccountInput {
            account_id: "EXTERNAL_PAYER".to_string(),
            institution: "External".to_string(),
            country: None,
            iban: None,
            bic: None,
            is_active: true,
        }),
        build_account(&AccountInput {
            account_id: "EXTERNAL_PAYEE".to_string(),
            institution: "External".to_string(),
            country: None,
            iban: None,
            bic: None,
            is_active: true,
        })
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_merge_with_no_duplicates() {
        let database = json!({
            "accounts": [
                {"account_id": "ACC001", "institution": "Bank A"}
            ]
        });

        let new_accounts = vec![
            json!({"account_id": "ACC002", "institution": "Bank B"}),
            json!({"account_id": "ACC003", "institution": "Bank C"}),
        ];

        let (merged, stats) = merge_accounts_with_deduplication(database, new_accounts).unwrap();

        assert_eq!(stats.added, 2);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.total, 2);

        let accounts = merged.get("accounts").unwrap().as_array().unwrap();
        assert_eq!(accounts.len(), 3);
    }

    #[test]
    fn test_merge_with_duplicates() {
        let database = json!({
            "accounts": [
                {"account_id": "ACC001", "institution": "Bank A"},
                {"account_id": "ACC002", "institution": "Bank B"}
            ]
        });

        let new_accounts = vec![
            json!({"account_id": "ACC002", "institution": "Bank B"}), // duplicate
            json!({"account_id": "ACC003", "institution": "Bank C"}), // new
        ];

        let (merged, stats) = merge_accounts_with_deduplication(database, new_accounts).unwrap();

        assert_eq!(stats.added, 1);
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.total, 2);
        assert!(stats.has_duplicates());

        let accounts = merged.get("accounts").unwrap().as_array().unwrap();
        assert_eq!(accounts.len(), 3); // Should still be 3, not 4
    }

    #[test]
    fn test_find_duplicate_account_ids() {
        let database = json!({
            "accounts": [
                {"account_id": "ACC001", "institution": "Bank A"},
                {"account_id": "ACC002", "institution": "Bank B"}
            ]
        });

        let check_ids = vec![
            "ACC001".to_string(),
            "ACC003".to_string(),
            "ACC002".to_string(),
        ];

        let duplicates = find_duplicate_account_ids(&database, &check_ids).unwrap();

        assert_eq!(duplicates.len(), 2);
        assert!(duplicates.contains(&"ACC001".to_string()));
        assert!(duplicates.contains(&"ACC002".to_string()));
        assert!(!duplicates.contains(&"ACC003".to_string()));
    }
}
