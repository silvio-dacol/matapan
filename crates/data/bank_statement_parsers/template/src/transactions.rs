//! Empty transaction scaffold for new parser crates.
//!
//! This file intentionally contains *no actual parsing logic*.
//! Copy this crate, then implement your bank-specific mapping by filling the
//! TODO blocks below.

use anyhow::Result;
use serde_json::Value;
use std::io::Read;

use crate::TemplateCsvParser;

pub fn parse_transactions<R: Read>(
    parser: &TemplateCsvParser,
    reader: R,
) -> Result<(Vec<Value>, Vec<String>)> {
    let _ = parser;
    let _ = reader;

    // TODO STEP 1: Create a bank-specific row struct:
    //   #[derive(Debug, Deserialize)]
    //   struct BankRow { ... }
    //
    // TODO STEP 2: Build a csv::ReaderBuilder and deserialize rows.
    //
    // TODO STEP 3: For each row, map fields to utils::TransactionInput:
    //   - date (YYYY-MM-DD)
    //   - from_account_id
    //   - to_account_id
    //   - transaction_type
    //   - category
    //   - amount (positive value)
    //   - currency
    //   - description
    //   - description_en (optional)
    //   - txn_id (source id or deterministic hash)
    //
    // TODO STEP 4: build normalized rows via:
    //   utils::build_transaction(&TransactionInput { ... })
    //
    // TODO STEP 5: Track used account ids and return them together with txns.

    Ok((Vec::new(), Vec::new()))
}
