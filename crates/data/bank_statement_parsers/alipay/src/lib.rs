use anyhow::Result;
use serde_json::{json, Value};
use std::io::Read;

mod accounts;
mod instruments;
mod positions;
mod transactions;

pub const PARSER_NAME: &str = "alipay";

pub struct AlipayCsvParser {
    pub account_id: String,
    pub currency: String,
    pub only_successful: bool,
}

impl AlipayCsvParser {
    pub fn new(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            currency: "CNY".to_string(),
            only_successful: true,
        }
    }

    pub fn with_currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = currency.into();
        self
    }

    pub fn with_only_successful(mut self, only_successful: bool) -> Self {
        self.only_successful = only_successful;
        self
    }

    /// Creates account entries for the Alipay accounts used by this parser.
    pub fn create_accounts(&self) -> Vec<Value> {
        accounts::create_accounts(self)
    }

    #[allow(dead_code)]
    pub fn create_account(&self) -> Value {
        self.create_accounts()
            .into_iter()
            .next()
            .unwrap_or_else(|| json!({}))
    }

    pub fn parse_reader<R: Read>(&self, reader: R) -> Result<Vec<Value>> {
        transactions::parse_transactions(self, reader)
    }
}

/// Merges Alipay transactions into an existing database.json Value.
/// Assumes database.json has a top level "transactions": [] array.
/// Automatically skips duplicate transactions based on txn_id.
pub fn merge_transactions_into_template(
    template: Value,
    new_txns: Vec<Value>,
) -> Result<(Value, utils::transactions::MergeStats)> {
    utils::merge_transactions_with_deduplication(template, new_txns)
}

/// Merges Alipay account entries into an existing database.json Value.
/// Assumes database.json has a top level "accounts": [] array.
/// Automatically skips duplicate accounts based on account_id.
pub fn merge_accounts_into_template(
    template: Value,
    new_accounts: Vec<Value>,
) -> Result<(Value, utils::accounts::MergeStats)> {
    utils::merge_accounts_with_deduplication(template, new_accounts)
}
