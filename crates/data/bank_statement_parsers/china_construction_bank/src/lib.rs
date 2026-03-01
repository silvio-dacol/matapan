use anyhow::Result;
use serde_json::Value;

mod accounts;
mod instruments;
mod positions;
mod transactions;

pub const PARSER_NAME: &str = "ccb";

pub struct CcbXlsParser {
    pub account_id: String,
    pub only_nonempty_rows: bool,
}

impl CcbXlsParser {
    pub fn new(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            only_nonempty_rows: true,
        }
    }

    pub fn with_only_nonempty_rows(mut self, only_nonempty_rows: bool) -> Self {
        self.only_nonempty_rows = only_nonempty_rows;
        self
    }

    pub fn create_accounts(&self) -> Vec<Value> {
        accounts::create_accounts(self)
    }

    #[allow(dead_code)]
    pub fn create_account(&self) -> Value {
        self.create_accounts()
            .into_iter()
            .next()
            .unwrap_or_default()
    }

    pub fn parse_file(&self, xls_path: &str) -> Result<Vec<Value>> {
        transactions::parse_transactions(self, xls_path)
    }
}

/// Merges CCB transactions into an existing database.json Value.
/// Assumes database.json has a top level "transactions": [] array.
/// Automatically skips duplicate transactions based on txn_id.
pub fn merge_transactions_into_template(
    template: Value,
    new_txns: Vec<Value>,
) -> Result<(Value, utils::transactions::MergeStats)> {
    utils::merge_transactions_with_deduplication(template, new_txns)
}

/// Merges CCB account entries into an existing database.json Value.
/// Assumes database.json has a top level "accounts": [] array.
/// Automatically skips duplicate accounts based on account_id.
pub fn merge_accounts_into_template(
    template: Value,
    new_accounts: Vec<Value>,
) -> Result<(Value, utils::accounts::MergeStats)> {
    utils::merge_accounts_with_deduplication(template, new_accounts)
}
