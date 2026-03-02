use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::{
    build_txn_id, cashflow_to_txn, parse_f64, parse_f64_opt, parse_ibkr_datetime_date,
    parse_yyyy_mm_dd, IbkrCsvParser, SectionHeader,
};

pub fn parse_transaction_row(
    parser: &IbkrCsvParser,
    section: &str,
    headers: &HashMap<String, SectionHeader>,
    row: &[String],
    transactions: &mut Vec<Value>,
) -> Result<bool> {
    if parse_deposits_and_withdrawals(parser, section, headers, row, transactions)? {
        return Ok(true);
    }
    if parse_fees(parser, section, headers, row, transactions)? {
        return Ok(true);
    }
    if parse_interest(parser, section, headers, row, transactions)? {
        return Ok(true);
    }
    if parse_withholding_tax(parser, section, headers, row, transactions)? {
        return Ok(true);
    }
    if parse_trades(parser, section, headers, row, transactions)? {
        return Ok(true);
    }
    if parse_corporate_actions(parser, section, headers, row, transactions)? {
        return Ok(true);
    }

    Ok(false)
}

fn parse_deposits_and_withdrawals(
    parser: &IbkrCsvParser,
    section: &str,
    headers: &HashMap<String, SectionHeader>,
    row: &[String],
    transactions: &mut Vec<Value>,
) -> Result<bool> {
    if section != "Deposits & Withdrawals" {
        return Ok(false);
    }

    let header = headers
        .get(section)
        .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

    let currency = field_dw_currency(header, row);
    let settle_date = field_dw_settle_date(header, row);
    let description = field_dw_description(header, row);
    let amount_raw = field_dw_amount_raw(header, row);

    if field_dw_skip_row(&currency, &settle_date) {
        return Ok(true);
    }

    let date = parse_yyyy_mm_dd(&settle_date).with_context(|| {
        format!(
            "Bad settle date '{}' in Deposits & Withdrawals",
            settle_date
        )
    })?;

    let signed = parse_f64(amount_raw)
        .with_context(|| format!("Bad amount '{}' in Deposits & Withdrawals", amount_raw))?;
    let (txn_type, from_acc, to_acc, amount) = cashflow_to_txn(&parser.account_id_checking, signed);

    let output_description = field_dw_output_description(&description);
    let txn_id = build_txn_id(
        "IBKR-DW",
        date,
        amount,
        &currency,
        &output_description,
        &settle_date,
    );

    transactions.push(build_transaction_value(TransactionValueInput {
        date: date.format("%Y-%m-%d").to_string(),
        from_account_id: from_acc,
        to_account_id: to_acc,
        transaction_type: txn_type,
        amount,
        currency,
        description: output_description,
        txn_id,
    }));

    Ok(true)
}

fn parse_fees(
    parser: &IbkrCsvParser,
    section: &str,
    headers: &HashMap<String, SectionHeader>,
    row: &[String],
    transactions: &mut Vec<Value>,
) -> Result<bool> {
    if section != "Fees" {
        return Ok(false);
    }

    let header = headers
        .get(section)
        .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

    let currency = field_fee_currency(header, row);
    let date_raw = field_fee_date_raw(header, row);
    let description = field_fee_description(header, row);
    let amount_raw = field_fee_amount_raw(header, row);

    if field_fee_skip_row(&currency, &date_raw) {
        return Ok(true);
    }

    let date = parse_yyyy_mm_dd(&date_raw).with_context(|| format!("Bad date '{}' in Fees", date_raw))?;
    let signed = parse_f64(amount_raw).with_context(|| format!("Bad amount '{}' in Fees", amount_raw))?;

    let normalized_signed = field_fee_normalized_signed_amount(signed);
    let (txn_type, from_acc, to_acc, amount) =
        cashflow_to_txn(&parser.account_id_checking, normalized_signed);

    let output_description = field_fee_output_description(&description);
    let txn_id = build_txn_id("IBKR-FEE", date, amount, &currency, &output_description, &date_raw);

    transactions.push(build_transaction_value(TransactionValueInput {
        date: date.format("%Y-%m-%d").to_string(),
        from_account_id: from_acc,
        to_account_id: to_acc,
        transaction_type: txn_type,
        amount,
        currency,
        description: output_description,
        txn_id,
    }));

    Ok(true)
}

fn parse_interest(
    parser: &IbkrCsvParser,
    section: &str,
    headers: &HashMap<String, SectionHeader>,
    row: &[String],
    transactions: &mut Vec<Value>,
) -> Result<bool> {
    if section != "Interest" {
        return Ok(false);
    }

    let header = headers
        .get(section)
        .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

    let currency = field_interest_currency(header, row);
    let date_raw = field_interest_date_raw(header, row);
    let description = field_interest_description(header, row);
    let amount_raw = field_interest_amount_raw(header, row);

    if field_interest_skip_row(&currency, &date_raw) {
        return Ok(true);
    }

    let date =
        parse_yyyy_mm_dd(&date_raw).with_context(|| format!("Bad date '{}' in Interest", date_raw))?;
    let signed =
        parse_f64(amount_raw).with_context(|| format!("Bad amount '{}' in Interest", amount_raw))?;

    let (txn_type, from_acc, to_acc, amount) = cashflow_to_txn(&parser.account_id_checking, signed);
    let output_description = field_interest_output_description(&description);
    let txn_id = build_txn_id("IBKR-INT", date, amount, &currency, &output_description, &date_raw);

    transactions.push(build_transaction_value(TransactionValueInput {
        date: date.format("%Y-%m-%d").to_string(),
        from_account_id: from_acc,
        to_account_id: to_acc,
        transaction_type: txn_type,
        amount,
        currency,
        description: output_description,
        txn_id,
    }));

    Ok(true)
}

fn parse_withholding_tax(
    parser: &IbkrCsvParser,
    section: &str,
    headers: &HashMap<String, SectionHeader>,
    row: &[String],
    transactions: &mut Vec<Value>,
) -> Result<bool> {
    if section != "Withholding Tax" {
        return Ok(false);
    }

    let header = headers
        .get(section)
        .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

    let currency = field_wht_currency(header, row);
    let date_raw = field_wht_date_raw(header, row);
    let description = field_wht_description(header, row);
    let amount_raw = field_wht_amount_raw(header, row);

    if field_wht_skip_row(&currency, &date_raw) {
        return Ok(true);
    }

    let date = parse_yyyy_mm_dd(&date_raw)
        .with_context(|| format!("Bad date '{}' in Withholding Tax", date_raw))?;
    let signed = parse_f64(amount_raw)
        .with_context(|| format!("Bad amount '{}' in Withholding Tax", amount_raw))?;

    let normalized_signed = field_wht_normalized_signed_amount(signed);
    let (txn_type, from_acc, to_acc, amount) =
        cashflow_to_txn(&parser.account_id_checking, normalized_signed);

    let output_description = field_wht_output_description(&description);
    let txn_id = build_txn_id("IBKR-WHT", date, amount, &currency, &output_description, &date_raw);

    transactions.push(build_transaction_value(TransactionValueInput {
        date: date.format("%Y-%m-%d").to_string(),
        from_account_id: from_acc,
        to_account_id: to_acc,
        transaction_type: txn_type,
        amount,
        currency,
        description: output_description,
        txn_id,
    }));

    Ok(true)
}

fn parse_trades(
    parser: &IbkrCsvParser,
    section: &str,
    headers: &HashMap<String, SectionHeader>,
    row: &[String],
    transactions: &mut Vec<Value>,
) -> Result<bool> {
    if section != "Trades" {
        return Ok(false);
    }

    let header = headers
        .get(section)
        .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

    let currency = field_trade_currency(header, row);
    let symbol = field_trade_symbol(header, row);
    let date_time_raw = field_trade_date_time_raw(header, row);
    let quantity_raw = field_trade_quantity_raw(header, row);
    let price_raw = field_trade_price_raw(header, row);
    let proceeds_raw = field_trade_proceeds_raw(header, row);
    let fee_raw = field_trade_fee_raw(header, row);

    let date = parse_ibkr_datetime_date(&date_time_raw)
        .with_context(|| format!("Bad Date/Time '{}' in Trades", date_time_raw))?;

    let proceeds = match parse_f64_opt(proceeds_raw) {
        Some(value) if value.abs() > 0.0 => value,
        _ => return Ok(true),
    };

    let (txn_type, from_acc, to_acc, amount_abs) = field_trade_transfer_direction(parser, proceeds);
    let output_description = field_trade_output_description(&symbol, quantity_raw, price_raw);
    let txn_id = build_txn_id(
        "IBKR-TRD",
        date,
        amount_abs,
        &currency,
        &output_description,
        &date_time_raw,
    );

    transactions.push(build_transaction_value(TransactionValueInput {
        date: date.format("%Y-%m-%d").to_string(),
        from_account_id: from_acc,
        to_account_id: to_acc,
        transaction_type: txn_type,
        amount: amount_abs,
        currency: currency.clone(),
        description: output_description,
        txn_id,
    }));

    if let Some(fee) = parse_f64_opt(fee_raw) {
        if fee.abs() > 0.0 {
            let fee_description = field_trade_fee_description(&symbol);
            let fee_txn_id = build_txn_id(
                "IBKR-TRFEE",
                date,
                fee.abs(),
                &currency,
                &fee_description,
                &date_time_raw,
            );

            transactions.push(build_transaction_value(TransactionValueInput {
                date: date.format("%Y-%m-%d").to_string(),
                from_account_id: parser.account_id_savings.clone(),
                to_account_id: "EXTERNAL_PAYEE".to_string(),
                transaction_type: "expense".to_string(),
                amount: fee.abs(),
                currency,
                description: fee_description,
                txn_id: fee_txn_id,
            }));
        }
    }

    Ok(true)
}

fn parse_corporate_actions(
    parser: &IbkrCsvParser,
    section: &str,
    headers: &HashMap<String, SectionHeader>,
    row: &[String],
    transactions: &mut Vec<Value>,
) -> Result<bool> {
    if section != "Corporate Actions" {
        return Ok(false);
    }

    let header = headers
        .get(section)
        .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

    let currency = field_corp_currency(header, row);
    let date_time_raw = field_corp_date_time_raw(header, row);
    let report_date_raw = field_corp_report_date_raw(header, row);
    let description = field_corp_description(header, row);
    let proceeds_raw = field_corp_proceeds_raw(header, row);

    if field_corp_skip_row(&currency) {
        return Ok(true);
    }

    let proceeds = match parse_f64_opt(proceeds_raw) {
        Some(value) if value.abs() > 0.0 => value,
        _ => return Ok(true),
    };

    let date = parse_ibkr_datetime_date(&date_time_raw)
        .or_else(|_| parse_yyyy_mm_dd(&report_date_raw))
        .with_context(|| format!("Bad Date/Time '{}' in Corporate Actions", date_time_raw))?;

    let (txn_type, from_acc, to_acc, amount_abs) = cashflow_to_txn(&parser.account_id_checking, proceeds);
    let output_description = field_corp_output_description(&description);
    let txn_id = build_txn_id(
        "IBKR-CA",
        date,
        amount_abs,
        &currency,
        &output_description,
        &date_time_raw,
    );

    transactions.push(build_transaction_value(TransactionValueInput {
        date: date.format("%Y-%m-%d").to_string(),
        from_account_id: from_acc,
        to_account_id: to_acc,
        transaction_type: txn_type,
        amount: amount_abs,
        currency,
        description: output_description,
        txn_id,
    }));

    Ok(true)
}

struct TransactionValueInput {
    date: String,
    from_account_id: String,
    to_account_id: String,
    transaction_type: String,
    amount: f64,
    currency: String,
    description: String,
    txn_id: String,
}

fn build_transaction_value(input: TransactionValueInput) -> Value {
    json!({
        "date": input.date,
        "from_account_id": input.from_account_id,
        "to_account_id": input.to_account_id,
        "type": input.transaction_type,
        "category": "uncategorized",
        "amount": input.amount,
        "currency": input.currency,
        "description": input.description,
        "txn_id": input.txn_id
    })
}

fn field_dw_currency(header: &SectionHeader, row: &[String]) -> String {
    header.get(row, "Currency").unwrap_or("").trim().to_string()
}

fn field_dw_settle_date(header: &SectionHeader, row: &[String]) -> String {
    header
        .get(row, "Settle Date")
        .unwrap_or("")
        .trim()
        .to_string()
}

fn field_dw_description(header: &SectionHeader, row: &[String]) -> String {
    header
        .get(row, "Description")
        .unwrap_or("")
        .trim()
        .to_string()
}

fn field_dw_amount_raw<'a>(header: &'a SectionHeader, row: &'a [String]) -> &'a str {
    header.get(row, "Amount").unwrap_or("").trim()
}

fn field_dw_skip_row(currency: &str, settle_date: &str) -> bool {
    currency.eq_ignore_ascii_case("total") || settle_date.is_empty()
}

fn field_dw_output_description(description: &str) -> String {
    format!("IBKR D/W: {}", description)
}

fn field_fee_currency(header: &SectionHeader, row: &[String]) -> String {
    header.get(row, "Currency").unwrap_or("").trim().to_string()
}

fn field_fee_date_raw(header: &SectionHeader, row: &[String]) -> String {
    header.get(row, "Date").unwrap_or("").trim().to_string()
}

fn field_fee_description(header: &SectionHeader, row: &[String]) -> String {
    header
        .get(row, "Description")
        .unwrap_or("")
        .trim()
        .to_string()
}

fn field_fee_amount_raw<'a>(header: &'a SectionHeader, row: &'a [String]) -> &'a str {
    header.get(row, "Amount").unwrap_or("").trim()
}

fn field_fee_skip_row(currency: &str, date_raw: &str) -> bool {
    currency.is_empty() || date_raw.is_empty()
}

fn field_fee_normalized_signed_amount(signed: f64) -> f64 {
    if signed > 0.0 {
        -signed
    } else {
        signed
    }
}

fn field_fee_output_description(description: &str) -> String {
    format!("IBKR Fee: {}", description)
}

fn field_interest_currency(header: &SectionHeader, row: &[String]) -> String {
    header.get(row, "Currency").unwrap_or("").trim().to_string()
}

fn field_interest_date_raw(header: &SectionHeader, row: &[String]) -> String {
    header.get(row, "Date").unwrap_or("").trim().to_string()
}

fn field_interest_description(header: &SectionHeader, row: &[String]) -> String {
    header
        .get(row, "Description")
        .unwrap_or("")
        .trim()
        .to_string()
}

fn field_interest_amount_raw<'a>(header: &'a SectionHeader, row: &'a [String]) -> &'a str {
    header.get(row, "Amount").unwrap_or("").trim()
}

fn field_interest_skip_row(currency: &str, date_raw: &str) -> bool {
    currency.eq_ignore_ascii_case("total") || date_raw.is_empty()
}

fn field_interest_output_description(description: &str) -> String {
    format!("IBKR Interest: {}", description)
}

fn field_wht_currency(header: &SectionHeader, row: &[String]) -> String {
    header.get(row, "Currency").unwrap_or("").trim().to_string()
}

fn field_wht_date_raw(header: &SectionHeader, row: &[String]) -> String {
    header.get(row, "Date").unwrap_or("").trim().to_string()
}

fn field_wht_description(header: &SectionHeader, row: &[String]) -> String {
    header
        .get(row, "Description")
        .unwrap_or("")
        .trim()
        .to_string()
}

fn field_wht_amount_raw<'a>(header: &'a SectionHeader, row: &'a [String]) -> &'a str {
    header.get(row, "Amount").unwrap_or("").trim()
}

fn field_wht_skip_row(currency: &str, date_raw: &str) -> bool {
    currency.eq_ignore_ascii_case("total") || date_raw.is_empty()
}

fn field_wht_normalized_signed_amount(signed: f64) -> f64 {
    if signed > 0.0 {
        -signed
    } else {
        signed
    }
}

fn field_wht_output_description(description: &str) -> String {
    format!("IBKR Withholding: {}", description)
}

fn field_trade_currency(header: &SectionHeader, row: &[String]) -> String {
    header.get(row, "Currency").unwrap_or("").trim().to_string()
}

fn field_trade_symbol(header: &SectionHeader, row: &[String]) -> String {
    header.get(row, "Symbol").unwrap_or("").trim().to_string()
}

fn field_trade_date_time_raw(header: &SectionHeader, row: &[String]) -> String {
    header
        .get(row, "Date/Time")
        .unwrap_or("")
        .trim()
        .to_string()
}

fn field_trade_quantity_raw<'a>(header: &'a SectionHeader, row: &'a [String]) -> &'a str {
    header.get(row, "Quantity").unwrap_or("").trim()
}

fn field_trade_price_raw<'a>(header: &'a SectionHeader, row: &'a [String]) -> &'a str {
    header.get(row, "T. Price").unwrap_or("").trim()
}

fn field_trade_proceeds_raw<'a>(header: &'a SectionHeader, row: &'a [String]) -> &'a str {
    header.get(row, "Proceeds").unwrap_or("").trim()
}

fn field_trade_fee_raw<'a>(header: &'a SectionHeader, row: &'a [String]) -> &'a str {
    header.get(row, "Comm/Fee").unwrap_or("").trim()
}

fn field_trade_transfer_direction(
    parser: &IbkrCsvParser,
    proceeds: f64,
) -> (String, String, String, f64) {
    if proceeds >= 0.0 {
        (
            "transfer".to_string(),
            parser.account_id_savings.clone(),
            parser.account_id_checking.clone(),
            proceeds.abs(),
        )
    } else {
        (
            "transfer".to_string(),
            parser.account_id_checking.clone(),
            parser.account_id_savings.clone(),
            proceeds.abs(),
        )
    }
}

fn field_trade_output_description(symbol: &str, quantity_raw: &str, price_raw: &str) -> String {
    format!(
        "Trade {} qty={} price={}",
        symbol,
        if quantity_raw.is_empty() { "?" } else { quantity_raw },
        if price_raw.is_empty() { "?" } else { price_raw }
    )
}

fn field_trade_fee_description(symbol: &str) -> String {
    format!("Trade fee {}", symbol)
}

fn field_corp_currency(header: &SectionHeader, row: &[String]) -> String {
    header.get(row, "Currency").unwrap_or("").trim().to_string()
}

fn field_corp_date_time_raw(header: &SectionHeader, row: &[String]) -> String {
    header
        .get(row, "Date/Time")
        .unwrap_or("")
        .trim()
        .to_string()
}

fn field_corp_report_date_raw(header: &SectionHeader, row: &[String]) -> String {
    header
        .get(row, "Report Date")
        .unwrap_or("")
        .trim()
        .to_string()
}

fn field_corp_description(header: &SectionHeader, row: &[String]) -> String {
    header
        .get(row, "Description")
        .unwrap_or("")
        .trim()
        .to_string()
}

fn field_corp_proceeds_raw<'a>(header: &'a SectionHeader, row: &'a [String]) -> &'a str {
    header.get(row, "Proceeds").unwrap_or("").trim()
}

fn field_corp_skip_row(currency: &str) -> bool {
    currency.is_empty()
}

fn field_corp_output_description(description: &str) -> String {
    format!("CorpAction: {}", description)
}
