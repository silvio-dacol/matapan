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
        structural_type: field_structural_type(),
        institution: field_institution(),
        country: field_country(),
        iban: field_iban(),
        bic: field_bic(),
        account_number: field_account_number(),
        owner: field_owner(),
        is_liability: field_is_liability(),
        supports_positions: field_supports_positions(),
        opened_date: field_opened_date(),
        closed_date: field_closed_date(),
        is_active: field_is_active(),
        notes: field_notes_current(),
    })
}

fn build_savings_account(parser: &RevolutCsvParser) -> Value {
    build_account(&AccountInput {
        account_id: field_account_id_savings(parser),
        structural_type: field_structural_type(),
        institution: field_institution(),
        country: field_country(),
        iban: field_iban(),
        bic: field_bic(),
        account_number: field_account_number(),
        owner: field_owner(),
        is_liability: field_is_liability(),
        supports_positions: field_supports_positions(),
        opened_date: field_opened_date(),
        closed_date: field_closed_date(),
        is_active: field_is_active(),
        notes: field_notes_savings(),
    })
}

fn field_account_id_current(parser: &RevolutCsvParser) -> String {
    parser.account_id_current.clone()
}

fn field_account_id_savings(parser: &RevolutCsvParser) -> String {
    parser.account_id_savings.clone()
}

fn field_structural_type() -> String {
    "bank".to_string()
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

fn field_account_number() -> Option<String> {
    None
}

fn field_owner() -> String {
    "self".to_string()
}

fn field_is_liability() -> bool {
    false
}

fn field_supports_positions() -> bool {
    false
}

fn field_opened_date() -> Option<String> {
    None
}

fn field_closed_date() -> Option<String> {
    None
}

fn field_is_active() -> bool {
    true
}

fn field_notes_current() -> Option<String> {
    Some("Revolut current account - some fields need manual completion".to_string())
}

fn field_notes_savings() -> Option<String> {
    Some("Revolut savings pocket - some fields need manual completion".to_string())
}
