use anyhow::Result;
use serde_json::Value;
use std::io::Read;

mod parser;

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
        parser::accounts::create_all_accounts(self)
    }

    pub fn parse_reader<R: Read>(&self, reader: R) -> Result<(Vec<Value>, Vec<String>)> {
        parser::transactions::parse_transactions(self, reader)
    }

    pub fn create_used_accounts(&self, used_account_ids: &[String]) -> Vec<Value> {
        parser::accounts::create_used_accounts(self, used_account_ids)
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
