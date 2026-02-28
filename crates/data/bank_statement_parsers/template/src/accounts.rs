//! Example account mapping scaffold.
//!
//! Replace all field_* helpers with bank-specific metadata.

use serde_json::Value;
use utils::{build_account, AccountInput};

use crate::TemplateCsvParser;

pub fn create_all_accounts(parser: &TemplateCsvParser) -> Vec<Value> {
    vec![build_primary_account(parser)]
}

pub fn create_used_accounts(parser: &TemplateCsvParser, used_account_ids: &[String]) -> Vec<Value> {
    let mut out = Vec::new();

    for account_id in used_account_ids {
        if account_id == &parser.account_id {
            out.push(build_primary_account(parser));
        }
    }

    out
}

fn build_primary_account(parser: &TemplateCsvParser) -> Value {
    build_account(&AccountInput {
        account_id: field_account_id(parser),
        institution: field_institution(),
        country: field_country(),
        iban: field_iban(),
        bic: field_bic(),
        is_active: field_is_active(),
    })
}

fn field_account_id(parser: &TemplateCsvParser) -> String {
    parser.account_id.clone()
}

fn field_institution() -> String {
    // TODO: replace with real institution name.
    "Template Bank".to_string()
}

fn field_country() -> Option<String> {
    None
}

fn field_iban() -> Option<String> {
    None
}

fn field_bic() -> Option<String> {
    None
}

fn field_is_active() -> bool {
    true
}
