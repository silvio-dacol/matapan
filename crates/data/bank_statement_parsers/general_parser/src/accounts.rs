use serde_json::Value;
use std::collections::HashSet;
use utils::{build_account, AccountInput};

use crate::GeneralParser;

pub fn create_all_accounts(parser: &GeneralParser) -> Vec<Value> {
    vec![build_primary_account(parser)]
}

pub fn create_used_accounts(parser: &GeneralParser, used_account_ids: &[String]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for account_id in used_account_ids {
        if !seen.insert(account_id.clone()) {
            continue;
        }

        if account_id == &parser.account_id {
            out.push(build_primary_account(parser));
        } else {
            out.push(build_account(&AccountInput {
                account_id: account_id.clone(),
                institution: parser.institution.clone(),
                country: None,
                iban: None,
                bic: None,
                is_active: true,
            }));
        }
    }

    if out.is_empty() {
        out.push(build_primary_account(parser));
    }

    out
}

fn build_primary_account(parser: &GeneralParser) -> Value {
    build_account(&AccountInput {
        account_id: parser.account_id.clone(),
        institution: parser.institution.clone(),
        country: None,
        iban: None,
        bic: None,
        is_active: true,
    })
}
