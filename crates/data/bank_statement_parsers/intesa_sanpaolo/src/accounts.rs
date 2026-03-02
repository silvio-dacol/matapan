use serde_json::Value;
use utils::{build_account, AccountInput};

use crate::IntesaSanpaoloParser;

pub fn create_all_accounts(parser: &IntesaSanpaoloParser) -> Vec<Value> {
    vec![build_checking_account(parser), build_trading_account(parser)]
}

fn build_checking_account(parser: &IntesaSanpaoloParser) -> Value {
    build_account(&AccountInput {
        account_id: field_checking_account_id(parser),
        institution: field_institution(),
        country: field_country(),
        iban: field_iban(),
        bic: field_bic(),
        is_active: field_is_active(),
    })
}

fn build_trading_account(parser: &IntesaSanpaoloParser) -> Value {
    build_account(&AccountInput {
        account_id: field_trading_account_id(parser),
        institution: field_institution(),
        country: field_country(),
        iban: field_iban(),
        bic: field_bic(),
        is_active: field_is_active(),
    })
}

fn field_checking_account_id(parser: &IntesaSanpaoloParser) -> String {
    parser.account_id_checking.clone()
}

fn field_trading_account_id(parser: &IntesaSanpaoloParser) -> String {
    parser.account_id_trading.clone()
}

fn field_institution() -> String {
    "Intesa Sanpaolo".to_string()
}

fn field_country() -> Option<String> {
    Some("IT".to_string())
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
