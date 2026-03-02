use serde_json::Value;
use utils::{build_account, AccountInput};

use crate::IbkrCsvParser;

pub fn create_all_accounts(parser: &IbkrCsvParser) -> Vec<Value> {
    vec![build_checking_account(parser), build_savings_account(parser)]
}

fn build_checking_account(parser: &IbkrCsvParser) -> Value {
    build_account(&AccountInput {
        account_id: field_checking_account_id(parser),
        institution: field_institution(),
        country: field_country(),
        iban: field_iban(),
        bic: field_bic(),
        is_active: field_is_active(),
    })
}

fn build_savings_account(parser: &IbkrCsvParser) -> Value {
    build_account(&AccountInput {
        account_id: field_savings_account_id(parser),
        institution: field_institution(),
        country: field_country(),
        iban: field_iban(),
        bic: field_bic(),
        is_active: field_is_active(),
    })
}

fn field_checking_account_id(parser: &IbkrCsvParser) -> String {
    parser.account_id_checking.clone()
}

fn field_savings_account_id(parser: &IbkrCsvParser) -> String {
    parser.account_id_savings.clone()
}

fn field_institution() -> String {
    "IBKR".to_string()
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
