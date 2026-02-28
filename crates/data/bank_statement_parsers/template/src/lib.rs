use anyhow::Result;
use serde_json::Value;
use std::io::Read;

mod accounts;
mod instruments;
mod positions;
mod transactions;

pub const PARSER_NAME: &str = "template";

pub struct TemplateCsvParser {
    pub account_id: String,
}

impl TemplateCsvParser {
    pub fn new(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
        }
    }

    pub fn create_accounts(&self) -> Vec<Value> {
        accounts::create_all_accounts(self)
    }

    pub fn parse_reader<R: Read>(&self, reader: R) -> Result<(Vec<Value>, Vec<String>)> {
        // Intentionally delegated to an empty scaffold implementation in
        // transactions.rs. Replace that module with real logic in copied crates.
        transactions::parse_transactions(self, reader)
    }

    pub fn create_used_accounts(&self, used_account_ids: &[String]) -> Vec<Value> {
        accounts::create_used_accounts(self, used_account_ids)
    }
}

/// Merges parsed transactions into an existing database.json Value.
/// Assumes database.json has a top level "transactions": [] array.
/// Automatically skips duplicate transactions based on txn_id.
///
/// # Returns
/// * `Result<(Value, utils::transactions::MergeStats)>` - The merged database and merge statistics
pub fn merge_transactions_into_template(
    template: Value,
    new_txns: Vec<Value>,
) -> Result<(Value, utils::transactions::MergeStats)> {
    utils::merge_transactions_with_deduplication(template, new_txns)
}

/// Merges account entries into an existing database.json Value.
/// Assumes database.json has a top level "accounts": [] array.
/// Automatically skips duplicate accounts based on account_id.
///
/// # Returns
/// * `Result<(Value, utils::accounts::MergeStats)>` - The merged database and merge statistics
pub fn merge_accounts_into_template(
    template: Value,
    new_accounts: Vec<Value>,
) -> Result<(Value, utils::accounts::MergeStats)> {
    utils::merge_accounts_with_deduplication(template, new_accounts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reader_is_empty_scaffold() {
        let csv = "Date,Description,Amount,Currency\n2026-01-05,Coffee,-4.50,EUR\n";

        let parser = TemplateCsvParser::new("TEMPLATE");
        let (txns, used_accounts) = parser.parse_reader(csv.as_bytes()).unwrap();

        assert!(txns.is_empty());
        assert!(used_accounts.is_empty());
    }

    #[test]
    fn create_used_accounts_returns_only_requested_accounts() {
        let parser = TemplateCsvParser::new("TEMPLATE");
        let accounts = parser.create_used_accounts(&["TEMPLATE".to_string()]);

        assert_eq!(accounts.len(), 1);
        let account_id = accounts[0]
            .get("account_id")
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(account_id, "TEMPLATE");
    }
}
