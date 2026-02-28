use anyhow::Result;
use serde_json::Value;
use std::io::Read;

mod accounts;
mod instruments;
mod positions;
mod transactions;

pub const PARSER_NAME: &str = "revolut";

pub struct RevolutCsvParser {
    pub account_id_current: String,
    pub account_id_savings: String,
    pub only_completed: bool,
}

impl RevolutCsvParser {
    pub fn new(account_id: impl Into<String>) -> Self {
        let input = account_id.into();

        let base = input
            .trim_end_matches("_CURRENT")
            .trim_end_matches("_SAVINGS")
            .to_string();

        Self {
            account_id_current: format!("{}_CURRENT", base),
            account_id_savings: format!("{}_SAVINGS", base),
            only_completed: true,
        }
    }

    pub fn with_only_completed(mut self, only_completed: bool) -> Self {
        self.only_completed = only_completed;
        self
    }

    pub fn create_accounts(&self) -> Vec<Value> {
        accounts::create_all_accounts(self)
    }

    pub fn parse_reader<R: Read>(&self, reader: R) -> Result<(Vec<Value>, Vec<String>)> {
        transactions::parse_transactions(self, reader)
    }

    pub fn create_used_accounts(&self, used_account_ids: &[String]) -> Vec<Value> {
        accounts::create_used_accounts(self, used_account_ids)
    }
}

/// Merges Revolut transactions into an existing database.json Value.
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

/// Merges Revolut account entries into an existing database.json Value.
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
    fn parse_reader_skips_non_completed_by_default() {
        let csv = "Type,Product,Started Date,Completed Date,Description,Amount,Fee,Currency,State,Balance\n\
Card Payment,Current,2026-01-05 10:00:00,2026-01-05 10:00:00,Coffee,-4.5,0,EUR,COMPLETED,100\n\
Card Payment,Current,2026-01-06 10:00:00,2026-01-06 10:00:00,Pending Item,-3.0,0,EUR,PENDING,97\n";

        let parser = RevolutCsvParser::new("REVOLUT");
        let (txns, used_accounts) = parser.parse_reader(csv.as_bytes()).unwrap();

        assert_eq!(txns.len(), 1);
        assert_eq!(used_accounts.len(), 1);
        assert!(used_accounts.contains(&"REVOLUT_CURRENT".to_string()));
    }

    #[test]
    fn parse_reader_tracks_savings_account_usage() {
        let csv = "Type,Product,Started Date,Completed Date,Description,Amount,Fee,Currency,State,Balance\n\
Transfer,Savings,2026-01-07 10:00:00,2026-01-07 10:00:00,Transfer from pocket,12.0,0,EUR,COMPLETED,250\n";

        let parser = RevolutCsvParser::new("REVOLUT");
        let (_txns, used_accounts) = parser.parse_reader(csv.as_bytes()).unwrap();

        assert_eq!(used_accounts.len(), 1);
        assert!(used_accounts.contains(&"REVOLUT_SAVINGS".to_string()));
    }

    #[test]
    fn create_used_accounts_returns_only_requested_accounts() {
        let parser = RevolutCsvParser::new("REVOLUT");
        let accounts = parser.create_used_accounts(&["REVOLUT_CURRENT".to_string()]);

        assert_eq!(accounts.len(), 1);
        let account_id = accounts[0]
            .get("account_id")
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(account_id, "REVOLUT_CURRENT");
    }
}
