use anyhow::Result;
use serde_json::Value;

mod accounts;
mod instruments;
mod positions;
mod transactions;

pub const PARSER_NAME: &str = "carpay";

pub struct CarPayXlsxParser {
    pub account_id: String,
    pub currency: String,
}

impl CarPayXlsxParser {
    pub fn new(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            currency: "SEK".to_string(),
        }
    }

    pub fn with_currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = currency.into();
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

    pub fn parse_file(&self, xlsx_path: &str) -> Result<Vec<Value>> {
        transactions::parse_transactions(self, xlsx_path)
    }
}

pub fn merge_transactions_into_template(
    template: Value,
    new_txns: Vec<Value>,
) -> Result<(Value, utils::transactions::MergeStats)> {
    utils::merge_transactions_with_deduplication(template, new_txns)
}

pub fn merge_accounts_into_template(
    template: Value,
    new_accounts: Vec<Value>,
) -> Result<(Value, utils::accounts::MergeStats)> {
    utils::merge_accounts_with_deduplication(template, new_accounts)
}
