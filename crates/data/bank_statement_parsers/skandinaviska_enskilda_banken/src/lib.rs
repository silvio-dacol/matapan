use anyhow::Result;
use serde_json::Value;

mod accounts;
mod instruments;
mod positions;
mod transactions;

pub const PARSER_NAME: &str = "skandinaviska_enskilda_banken";

#[derive(Clone, Debug)]
pub(crate) struct SebMeta {
    // Normalized digits-only, e.g. "50200105205"
    pub account_number_digits: Option<String>,
}

pub struct SebXlsxParser {
    pub account_id_checking: String,
    pub account_id_savings: String,

    // Optional, but helps internal transfer mapping
    pub checking_account_number_digits: Option<String>,
    pub savings_account_number_digits: Option<String>,
}

impl SebXlsxParser {
    pub fn new(checking_account_id: impl Into<String>, savings_account_id: impl Into<String>) -> Self {
        Self {
            account_id_checking: checking_account_id.into(),
            account_id_savings: savings_account_id.into(),
            checking_account_number_digits: None,
            savings_account_number_digits: None,
        }
    }

    /// If you know the account numbers (digits-only) you can set them and improve internal transfer detection.
    /// Example digits-only: "50200105205", "50371807786"
    pub fn with_account_numbers(
        mut self,
        checking_digits: Option<String>,
        savings_digits: Option<String>,
    ) -> Self {
        self.checking_account_number_digits = checking_digits;
        self.savings_account_number_digits = savings_digits;
        self
    }

    pub fn create_accounts(&self) -> Vec<Value> {
        accounts::create_accounts(self)
    }

    pub fn parse_file(&self, path: &str, account_id: &str) -> Result<Vec<Value>> {
        transactions::parse_transactions(self, path, account_id)
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
