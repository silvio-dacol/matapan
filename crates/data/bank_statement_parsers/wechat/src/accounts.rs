use serde_json::Value;
use utils::{build_account, AccountInput};

use crate::WeChatXlsxParser;

pub fn create_accounts(parser: &WeChatXlsxParser) -> Vec<Value> {
    vec![build_account(&AccountInput {
        account_id: parser.account_id.clone(),
        institution: field_institution(),
        country: field_country(),
        iban: field_iban(),
        bic: field_bic(),
        is_active: field_is_active(),
    })]
}

fn field_institution() -> String {
    "WeChat Pay".to_string()
}

fn field_country() -> Option<String> {
    Some("CN".to_string())
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
