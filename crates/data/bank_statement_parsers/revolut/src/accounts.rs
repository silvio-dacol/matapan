//! Maps Revolut account variants (current/savings) into normalized account records.

use serde_json::Value;
use utils::{build_account, AccountInput};

use crate::RevolutCsvParser;

pub fn create_all_accounts(parser: &RevolutCsvParser) -> Vec<Value> {
    vec![
        build_current_account(parser),
        build_savings_account(parser),
    ]
}

pub fn create_used_accounts(parser: &RevolutCsvParser, used_account_ids: &[String]) -> Vec<Value> {
    let mut out = Vec::new();

    for account_id in used_account_ids {
        if account_id == &parser.account_id_current {
            out.push(build_current_account(parser));
        } else if account_id == &parser.account_id_savings {
            out.push(build_savings_account(parser));
        }
    }

    out
}

fn build_current_account(parser: &RevolutCsvParser) -> Value {
    build_account(&AccountInput {
        account_id: field_account_id_current(parser),
        institution: field_institution(),
        country: field_country(),
        iban: field_iban(),
        bic: field_bic(),
        is_active: field_is_active(),
    })
}

fn build_savings_account(parser: &RevolutCsvParser) -> Value {
    build_account(&AccountInput {
        account_id: field_account_id_savings(parser),
        institution: field_institution(),
        country: field_country(),
        iban: field_iban(),
        bic: field_bic(),
        is_active: field_is_active(),
    })
}

fn field_account_id_current(parser: &RevolutCsvParser) -> String {
    parser.account_id_current.clone()
}

fn field_account_id_savings(parser: &RevolutCsvParser) -> String {
    parser.account_id_savings.clone()
}

fn field_institution() -> String {
    "Revolut".to_string()
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
