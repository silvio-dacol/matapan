use anyhow::{anyhow, Context, Result};
use chrono::{Local, NaiveDate};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use utils::{build_position, PositionInput};

pub const PARSER_NAME: &str = "ibkr";

pub const IBKR_CHECKING: &str = "IBKR_CHECKING";
pub const IBKR_SAVINGS: &str = "IBKR_SAVINGS";

#[derive(Debug, Clone)]
struct SectionHeader {
    #[allow(dead_code)]
    cols: Vec<String>,
    idx: HashMap<String, usize>,
}

impl SectionHeader {
    fn new(cols: Vec<String>) -> Self {
        let mut idx = HashMap::new();
        for (i, c) in cols.iter().enumerate() {
            let k = c.trim().to_string();
            if !k.is_empty() {
                idx.insert(k, i);
            }
        }
        Self { cols, idx }
    }

    fn get<'a>(&self, row: &'a [String], col: &str) -> Option<&'a str> {
        let i = *self.idx.get(col)?;
        row.get(i).map(|s| s.as_str())
    }
}

pub struct IbkrCsvParser {
    pub account_id_checking: String,
    pub account_id_savings: String,
}

impl IbkrCsvParser {
    pub fn new() -> Self {
        Self {
            account_id_checking: IBKR_CHECKING.to_string(),
            account_id_savings: IBKR_SAVINGS.to_string(),
        }
    }

    pub fn create_accounts(&self) -> Vec<Value> {
        vec![
            json!({
                "account_id": self.account_id_checking,
                "structural_type": "bank",
                "institution": "IBKR",
                "country": null,
                "iban": null,
                "bic": null,
                "account_number": null,
                "owner": "self",
                "is_liability": false,
                "supports_positions": false,
                "opened_date": null,
                "closed_date": null,
                "is_active": true,
                "notes": "IBKR cash ledger (OPTION 1): all trade cash, fees, dividends, deposits/withdrawals settle here"
            }),
            json!({
                "account_id": self.account_id_savings,
                "structural_type": "brokerage",
                "institution": "IBKR",
                "country": null,
                "iban": null,
                "bic": null,
                "account_number": null,
                "owner": "self",
                "is_liability": false,
                "supports_positions": true,
                "opened_date": null,
                "closed_date": null,
                "is_active": true,
                "notes": "IBKR investment account: holds stock/ETF positions and receives interest income"
            }),
        ]
    }

    /// Parses:
    /// - transactions (cash only, OPTION 1)
    /// - instruments (from Financial Instrument Information)
    /// - positions (from Open Positions, as_of_date = statement end)
    pub fn parse_file<P: AsRef<Path>>(&self, path: P) -> Result<ParsedIbkr> {
        let fallback_statement_date = extract_statement_end_from_filename(path.as_ref())
            .or_else(|| extract_file_statement_date(path.as_ref()))
            .unwrap_or_else(|| Local::now().naive_local().date());
        let mut file = std::fs::File::open(path.as_ref())
            .with_context(|| format!("Cannot open {}", path.as_ref().display()))?;
        self.parse_reader_with_fallback_date(&mut file, Some(fallback_statement_date))
    }

    pub fn parse_reader<R: Read>(&self, reader: R) -> Result<ParsedIbkr> {
        self.parse_reader_with_fallback_date(reader, None)
    }

    fn parse_reader_with_fallback_date<R: Read>(
        &self,
        mut reader: R,
        fallback_statement_date: Option<NaiveDate>,
    ) -> Result<ParsedIbkr> {
        let mut buf = String::new();
        reader.read_to_string(&mut buf)?;

        let mut csvr = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(buf.as_bytes());

        // Track headers per section
        let mut headers: HashMap<String, SectionHeader> = HashMap::new();

        // Parsed outputs
        let mut statement_end: Option<NaiveDate> = None;

        let mut instruments: Vec<Value> = Vec::new();
        let mut positions: Vec<Value> = Vec::new();
        let mut transactions: Vec<Value> = Vec::new();

        // instrument lookup by (asset_category|symbol) -> instrument_id
        let mut instrument_key_to_id: HashMap<String, String> = HashMap::new();
        // remap old instrument_id -> new instrument_id when placeholders are replaced
        let mut instrument_id_remap: HashMap<String, String> = HashMap::new();

        for rec in csvr.records() {
            let rec = rec?;
            if rec.len() < 2 {
                continue;
            }

            let section = rec.get(0).unwrap_or("").trim();
            let row_kind = rec.get(1).unwrap_or("").trim();

            let mut row: Vec<String> = Vec::new();
            for i in 2..rec.len() {
                row.push(rec.get(i).unwrap_or("").to_string());
            }

            if row_kind == "Header" {
                // store header cols for the section
                let cols = row.iter().map(|s| s.trim().to_string()).collect::<Vec<_>>();
                headers.insert(section.to_string(), SectionHeader::new(cols));
                continue;
            }

            if row_kind != "Data" {
                continue;
            }

            // Statement end date
            if section.eq_ignore_ascii_case("Statement") {
                // columns: Field Name, Field Value
                let field_name = row.get(0).map(|s| s.trim()).unwrap_or("").to_lowercase();
                let field_val = row.get(1).map(|s| s.trim()).unwrap_or("");

                if field_name.contains("whengenerated") {
                    statement_end = parse_statement_generated(field_val).ok().or(statement_end);
                }
                if field_name.contains("period") {
                    statement_end = parse_statement_end(field_val).ok();
                }
                continue;
            }

            // Instruments
            if section == "Financial Instrument Information" {
                let h = headers
                    .get(section)
                    .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

                let asset_category = h
                    .get(&row, "Asset Category")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let symbol = h.get(&row, "Symbol").unwrap_or("").trim().to_string();
                let description = h.get(&row, "Description").unwrap_or("").trim().to_string();
                let conid = h.get(&row, "Conid").unwrap_or("").trim().to_string();
                let security_id = h.get(&row, "Security ID").unwrap_or("").trim().to_string();
                let itype = h.get(&row, "Type").unwrap_or("").trim().to_string();

                if symbol.is_empty() && conid.is_empty() && security_id.is_empty() {
                    continue;
                }

                let instrument_id =
                    build_instrument_id(&conid, &security_id, &asset_category, &primary_symbol(&symbol));

                let inst = json!({
                    "instrument_id": instrument_id,
                    "source": "IBKR",
                    "asset_category": null_if_empty(&asset_category),
                    "description": null_if_empty(&description),
                    "security_id": null_if_empty(&security_id),
                    "type": null_if_empty(&itype)
                });

                let aliases = split_symbol_aliases(&symbol);

                // Resolve existing instrument by any known alias key
                let mut existing_id: Option<String> = None;
                for alias in &aliases {
                    let key = format!("{}|{}", asset_category, alias);
                    if let Some(id) = instrument_key_to_id.get(&key) {
                        existing_id = Some(resolve_instrument_id(id, &instrument_id_remap));
                        break;
                    }
                }

                if existing_id.is_none() {
                    let full_symbol_key = format!("{}|{}", asset_category, symbol);
                    if let Some(id) = instrument_key_to_id.get(&full_symbol_key) {
                        existing_id = Some(resolve_instrument_id(id, &instrument_id_remap));
                    }
                }

                let final_instrument_id = if let Some(existing_id) = existing_id {
                    if let Some(idx) = instruments.iter().position(|i| {
                        i.get("instrument_id")
                            .and_then(|v| v.as_str())
                            .map(|id| id == existing_id)
                            .unwrap_or(false)
                    }) {
                        let merged = merge_instrument_entries(&instruments[idx], &inst);
                        let merged_asset_category = merged
                            .get("asset_category")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let merged_symbol = merged.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
                        let merged_conid = merged.get("conid").and_then(|v| v.as_str()).unwrap_or("");
                        let merged_security_id = merged
                            .get("security_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        let new_instrument_id = build_instrument_id(
                            merged_conid,
                            merged_security_id,
                            merged_asset_category,
                            &primary_symbol(merged_symbol),
                        );

                        let mut merged_with_id = merged;
                        if let Some(id_field) = merged_with_id.get_mut("instrument_id") {
                            *id_field = Value::String(new_instrument_id.clone());
                        }

                        instruments[idx] = merged_with_id;

                        if new_instrument_id != existing_id {
                            instrument_id_remap.insert(existing_id.clone(), new_instrument_id.clone());
                        }

                        new_instrument_id
                    } else {
                        let id = inst
                            .get("instrument_id")
                            .and_then(|v| v.as_str())
                            .unwrap()
                            .to_string();
                        instruments.push(inst);
                        id
                    }
                } else if let Some(idx) = instruments.iter().position(|i| {
                    i.get("instrument_id")
                        .and_then(|v| v.as_str())
                        .map(|id| id == instrument_id)
                        .unwrap_or(false)
                }) {
                    instruments[idx] = merge_instrument_entries(&instruments[idx], &inst);
                    instrument_id.clone()
                } else {
                    let id = inst
                        .get("instrument_id")
                        .and_then(|v| v.as_str())
                        .unwrap()
                        .to_string();
                    instruments.push(inst);
                    id
                };

                // Index all aliases to the final instrument id
                for alias in &aliases {
                    let key = format!("{}|{}", asset_category, alias);
                    instrument_key_to_id.insert(key, final_instrument_id.clone());
                }

                let full_symbol_key = format!("{}|{}", asset_category, symbol);
                instrument_key_to_id.insert(full_symbol_key, final_instrument_id);

                continue;
            }

            // Positions
            if section == "Open Positions" {
                let h = headers
                    .get(section)
                    .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

                let asset_category = h
                    .get(&row, "Asset Category")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let currency = h.get(&row, "Currency").unwrap_or("").trim().to_string();
                let symbol = h.get(&row, "Symbol").unwrap_or("").trim().to_string();

                let qty = parse_f64_opt(h.get(&row, "Quantity").unwrap_or(""));
                if qty.is_none() {
                    continue;
                }

                let as_of = statement_end
                    .or(fallback_statement_date)
                    .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());

                // Link to instrument if possible, otherwise create a placeholder instrument
                let inst_key = format!("{}|{}", asset_category, symbol);
                let instrument_id = if let Some(id) = instrument_key_to_id.get(&inst_key) {
                    id.clone()
                } else {
                    // placeholder instrument from position row
                    let fallback_id = build_instrument_id("", "", &asset_category, &symbol);
                    let placeholder = json!({
                        "instrument_id": fallback_id,
                        "source": "IBKR",
                        "asset_category": null_if_empty(&asset_category),
                        "description": null,
                        "security_id": null,
                        "type": null
                    });
                    instruments.push(placeholder);
                    instrument_key_to_id.insert(inst_key, fallback_id.clone());
                    fallback_id
                };

                let cost_price = parse_f64_opt(h.get(&row, "Cost Price").unwrap_or(""));
                let cost_basis = parse_f64_opt(h.get(&row, "Cost Basis").unwrap_or(""));
                let close_price = parse_f64_opt(h.get(&row, "Close Price").unwrap_or(""));
                let value = parse_f64_opt(h.get(&row, "Value").unwrap_or(""));
                let upl = parse_f64_opt(h.get(&row, "Unrealized P/L").unwrap_or(""));

                let position_id = make_hash_id(&format!(
                    "{}|{}|{}",
                    self.account_id_savings,
                    as_of.format("%Y-%m-%d"),
                    instrument_id
                ));
                let currency_value = null_if_empty(&currency);
                let pos = build_position(
                    &PositionInput {
                        position_id: format!("IBKRPOS-{}", &position_id[..12]),
                        source: "IBKR".to_string(),
                        as_of_date: as_of.format("%Y-%m-%d").to_string(),
                        account_id: self.account_id_savings.clone(),
                        instrument_id: instrument_id.clone(),
                        quantity: qty,
                        currency: currency_value.as_str().map(|s| s.to_string()),
                        cost_price,
                        cost_basis,
                        close_price,
                        market_value: value,
                    },
                    upl,
                );

                positions.push(pos);
                continue;
            }

            // Transactions: Deposits & Withdrawals
            if section == "Deposits & Withdrawals" {
                let h = headers
                    .get(section)
                    .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

                let currency = h.get(&row, "Currency").unwrap_or("").trim().to_string();
                let settle_date = h.get(&row, "Settle Date").unwrap_or("").trim().to_string();
                let description = h.get(&row, "Description").unwrap_or("").trim().to_string();
                let amount_raw = h.get(&row, "Amount").unwrap_or("").trim();

                // Skip total rows
                if currency.eq_ignore_ascii_case("total") || settle_date.is_empty() {
                    continue;
                }

                let date = parse_yyyy_mm_dd(&settle_date).with_context(|| {
                    format!(
                        "Bad settle date '{}' in Deposits & Withdrawals",
                        settle_date
                    )
                })?;

                let signed = parse_f64(amount_raw).with_context(|| {
                    format!("Bad amount '{}' in Deposits & Withdrawals", amount_raw)
                })?;

                let (txn_type, from_acc, to_acc, amount) =
                    cashflow_to_txn(&self.account_id_checking, signed);

                let desc = format!("IBKR D/W: {}", description);
                let txn_id = build_txn_id("IBKR-DW", date, amount, &currency, &desc, &settle_date);

                transactions.push(json!({
                    "date": date.format("%Y-%m-%d").to_string(),
                    "from_account_id": from_acc,
                    "to_account_id": to_acc,
                    "type": txn_type,
                    "category": "uncategorized",
                    "amount": amount,
                    "currency": currency,
                    "description": desc,
                    "txn_id": txn_id
                }));
                continue;
            }

            // Transactions: Fees
            if section == "Fees" {
                let h = headers
                    .get(section)
                    .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

                let currency = h.get(&row, "Currency").unwrap_or("").trim().to_string();
                let date_raw = h.get(&row, "Date").unwrap_or("").trim().to_string();
                let description = h.get(&row, "Description").unwrap_or("").trim().to_string();
                let amount_raw = h.get(&row, "Amount").unwrap_or("").trim();

                // Skip total rows or rows with missing data
                if currency.is_empty() || date_raw.is_empty() {
                    continue;
                }

                let date = parse_yyyy_mm_dd(&date_raw)
                    .with_context(|| format!("Bad date '{}' in Fees", date_raw))?;
                let signed = parse_f64(amount_raw)
                    .with_context(|| format!("Bad amount '{}' in Fees", amount_raw))?;

                // Fees should be expenses
                let signed = if signed > 0.0 { -signed } else { signed };
                let (txn_type, from_acc, to_acc, amount) =
                    cashflow_to_txn(&self.account_id_checking, signed);

                let desc = format!("IBKR Fee: {}", description);
                let txn_id = build_txn_id("IBKR-FEE", date, amount, &currency, &desc, &date_raw);

                transactions.push(json!({
                    "date": date.format("%Y-%m-%d").to_string(),
                    "from_account_id": from_acc,
                    "to_account_id": to_acc,
                    "type": txn_type,
                    "category": "uncategorized",
                    "amount": amount,
                    "currency": currency,
                    "description": desc,
                    "txn_id": txn_id
                }));
                continue;
            }

            // Transactions: Interest (route to savings)
            if section == "Interest" {
                let h = headers
                    .get(section)
                    .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

                let currency = h.get(&row, "Currency").unwrap_or("").trim().to_string();
                let date_raw = h.get(&row, "Date").unwrap_or("").trim().to_string();
                let description = h.get(&row, "Description").unwrap_or("").trim().to_string();
                let amount_raw = h.get(&row, "Amount").unwrap_or("").trim();

                // Skip total rows or rows with missing data
                if currency.eq_ignore_ascii_case("total") || date_raw.is_empty() {
                    continue;
                }

                let date = parse_yyyy_mm_dd(&date_raw)
                    .with_context(|| format!("Bad date '{}' in Interest", date_raw))?;
                let signed = parse_f64(amount_raw)
                    .with_context(|| format!("Bad amount '{}' in Interest", amount_raw))?;

                let (txn_type, from_acc, to_acc, amount) =
                    cashflow_to_txn(&self.account_id_checking, signed);

                let desc = format!("IBKR Interest: {}", description);
                let txn_id = build_txn_id("IBKR-INT", date, amount, &currency, &desc, &date_raw);

                transactions.push(json!({
                    "date": date.format("%Y-%m-%d").to_string(),
                    "from_account_id": from_acc,
                    "to_account_id": to_acc,
                    "type": txn_type,
                    "category": "uncategorized",
                    "amount": amount,
                    "currency": currency,
                    "description": desc,
                    "txn_id": txn_id
                }));
                continue;
            }

            // Transactions: Withholding Tax
            if section == "Withholding Tax" {
                let h = headers
                    .get(section)
                    .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

                let currency = h.get(&row, "Currency").unwrap_or("").trim().to_string();
                let date_raw = h.get(&row, "Date").unwrap_or("").trim().to_string();
                let description = h.get(&row, "Description").unwrap_or("").trim().to_string();
                let amount_raw = h.get(&row, "Amount").unwrap_or("").trim();

                // Skip total rows or rows with missing data
                if currency.eq_ignore_ascii_case("total") || date_raw.is_empty() {
                    continue;
                }

                let date = parse_yyyy_mm_dd(&date_raw)
                    .with_context(|| format!("Bad date '{}' in Withholding Tax", date_raw))?;
                let signed = parse_f64(amount_raw)
                    .with_context(|| format!("Bad amount '{}' in Withholding Tax", amount_raw))?;

                // withholding should be expense
                let signed = if signed > 0.0 { -signed } else { signed };
                let (txn_type, from_acc, to_acc, amount) =
                    cashflow_to_txn(&self.account_id_checking, signed);

                let desc = format!("IBKR Withholding: {}", description);
                let txn_id = build_txn_id("IBKR-WHT", date, amount, &currency, &desc, &date_raw);

                transactions.push(json!({
                    "date": date.format("%Y-%m-%d").to_string(),
                    "from_account_id": from_acc,
                    "to_account_id": to_acc,
                    "type": txn_type,
                    "category": "uncategorized",
                    "amount": amount,
                    "currency": currency,
                    "description": desc,
                    "txn_id": txn_id
                }));
                continue;
            }

            // Transactions: Trades (cash impact on IBKR_CHECKING only, OPTION 1)
            if section == "Trades" {
                let h = headers
                    .get(section)
                    .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

                let currency = h.get(&row, "Currency").unwrap_or("").trim().to_string();
                let symbol = h.get(&row, "Symbol").unwrap_or("").trim().to_string();
                let dt_raw = h.get(&row, "Date/Time").unwrap_or("").trim().to_string();
                let qty_raw = h.get(&row, "Quantity").unwrap_or("").trim();
                let price_raw = h.get(&row, "T. Price").unwrap_or("").trim();
                let proceeds_raw = h.get(&row, "Proceeds").unwrap_or("").trim();
                let fee_raw = h.get(&row, "Comm/Fee").unwrap_or("").trim();

                // date/time in file looks like: 2025-11-27, 15:41:12
                let date = parse_ibkr_datetime_date(&dt_raw)
                    .with_context(|| format!("Bad Date/Time '{}' in Trades", dt_raw))?;

                let proceeds = match parse_f64_opt(proceeds_raw) {
                    Some(v) if v.abs() > 0.0 => v,
                    _ => continue,
                };

                // Trade proceeds flow between IBKR accounts:
                // - Buying (negative proceeds): CHECKING -> SAVINGS (cash moves to investment account)
                // - Selling (positive proceeds): SAVINGS -> CHECKING (cash returns from sale)
                let (txn_type, from_acc, to_acc, amount_abs) = if proceeds >= 0.0 {
                    // Selling: money comes back from investment account to checking
                    (
                        "transfer".to_string(),
                        self.account_id_savings.clone(),
                        self.account_id_checking.clone(),
                        proceeds.abs(),
                    )
                } else {
                    // Buying: money leaves checking to pay for investment
                    (
                        "transfer".to_string(),
                        self.account_id_checking.clone(),
                        self.account_id_savings.clone(),
                        proceeds.abs(),
                    )
                };

                let desc = format!(
                    "Trade {} qty={} price={}",
                    symbol,
                    if qty_raw.is_empty() { "?" } else { qty_raw },
                    if price_raw.is_empty() { "?" } else { price_raw }
                );
                let txn_id = build_txn_id("IBKR-TRD", date, amount_abs, &currency, &desc, &dt_raw);

                transactions.push(json!({
                    "date": date.format("%Y-%m-%d").to_string(),
                    "from_account_id": from_acc,
                    "to_account_id": to_acc,
                    "type": txn_type,
                    "category": "uncategorized",
                    "amount": amount_abs,
                    "currency": currency.clone(),
                    "description": desc,
                    "txn_id": txn_id
                }));

                // Fee line (optional) - fees are paid from the investment account
                if let Some(fee) = parse_f64_opt(fee_raw) {
                    if fee.abs() > 0.0 {
                        let desc2 = format!("Trade fee {}", symbol);
                        let txn_id2 =
                            build_txn_id("IBKR-TRFEE", date, fee.abs(), &currency, &desc2, &dt_raw);

                        transactions.push(json!({
                            "date": date.format("%Y-%m-%d").to_string(),
                            "from_account_id": self.account_id_savings,
                            "to_account_id": "EXTERNAL_PAYEE",
                            "type": "expense",
                            "category": "uncategorized",
                            "amount": fee.abs(),
                            "currency": currency,
                            "description": desc2,
                            "txn_id": txn_id2
                        }));
                    }
                }

                continue;
            }

            // Transactions: Corporate Actions (only if Proceeds exists and non-zero)
            if section == "Corporate Actions" {
                let h = headers
                    .get(section)
                    .ok_or_else(|| anyhow!("Missing header for section {}", section))?;

                let currency = h.get(&row, "Currency").unwrap_or("").trim().to_string();
                let dt_raw = h.get(&row, "Date/Time").unwrap_or("").trim().to_string();
                let description = h.get(&row, "Description").unwrap_or("").trim().to_string();
                let proceeds_raw = h.get(&row, "Proceeds").unwrap_or("").trim();

                // Skip total rows or rows with missing data
                if currency.is_empty() {
                    continue;
                }

                let proceeds = match parse_f64_opt(proceeds_raw) {
                    Some(v) if v.abs() > 0.0 => v,
                    _ => continue,
                };

                let date = parse_ibkr_datetime_date(&dt_raw)
                    .or_else(|_| {
                        // sometimes "Report Date" exists as yyyy-mm-dd
                        let rd = h.get(&row, "Report Date").unwrap_or("").trim();
                        parse_yyyy_mm_dd(rd)
                    })
                    .with_context(|| format!("Bad Date/Time '{}' in Corporate Actions", dt_raw))?;

                let (txn_type, from_acc, to_acc, amount_abs) =
                    cashflow_to_txn(&self.account_id_checking, proceeds);

                let desc = format!("CorpAction: {}", description);
                let txn_id = build_txn_id("IBKR-CA", date, amount_abs, &currency, &desc, &dt_raw);

                transactions.push(json!({
                    "date": date.format("%Y-%m-%d").to_string(),
                    "from_account_id": from_acc,
                    "to_account_id": to_acc,
                    "type": txn_type,
                    "category": "uncategorized",
                    "amount": amount_abs,
                    "currency": currency,
                    "description": desc,
                    "txn_id": txn_id
                }));
                continue;
            }
        }

        // After all sections are processed, update positions with corrected instrument IDs
        // (in case placeholders were created first and later updated with ISIN/conid-based IDs)
        for pos in positions.iter_mut() {
            if let Some(old_inst_id) = pos.get("instrument_id").and_then(|v| v.as_str()) {
                let correct_id = resolve_instrument_id(old_inst_id, &instrument_id_remap);
                if correct_id != old_inst_id {
                    if let Some(inst_id_field) = pos.get_mut("instrument_id") {
                        *inst_id_field = Value::String(correct_id.clone());
                    }
                    // Also update position_id since it includes the instrument_id
                    if let Some(as_of) = pos.get("as_of_date").and_then(|v| v.as_str()) {
                        let new_position_id = make_hash_id(&format!(
                            "{}|{}|{}",
                            self.account_id_savings, as_of, correct_id
                        ));
                        if let Some(pos_id_field) = pos.get_mut("position_id") {
                            *pos_id_field =
                                Value::String(format!("IBKRPOS-{}", &new_position_id[..12]));
                        }
                    }
                }
            }
        }

        Ok(ParsedIbkr {
            statement_end,
            instruments,
            positions,
            transactions,
        })
    }
}

pub struct ParsedIbkr {
    pub statement_end: Option<NaiveDate>,
    pub instruments: Vec<Value>,
    pub positions: Vec<Value>,
    pub transactions: Vec<Value>,
}

// ------------------------
// Merging helpers (local)
// ------------------------

pub fn merge_instruments_with_deduplication(
    template: Value,
    new_instruments: Vec<Value>,
) -> Result<(Value, utils::MergeStats)> {
    utils::merge_instruments_with_deduplication(template, new_instruments)
}

// ------------------------
// Parsing helpers
// ------------------------

fn null_if_empty(s: &str) -> Value {
    if s.trim().is_empty() {
        Value::Null
    } else {
        Value::String(s.to_string())
    }
}

fn parse_statement_end(period: &str) -> Result<NaiveDate> {
    // e.g. "December 1, 2025 - January 30, 2026"
    let normalized = period
        .trim()
        .trim_matches('"')
        .replace('–', "-")
        .replace('—', "-");

    if let Some((_, end)) = normalized.split_once('-') {
        return parse_flexible_date(end.trim());
    }

    parse_flexible_date(&normalized)
}

fn parse_statement_generated(value: &str) -> Result<NaiveDate> {
    parse_ibkr_datetime_date(value)
}

fn extract_file_statement_date(path: &Path) -> Option<NaiveDate> {
    let metadata = std::fs::metadata(path).ok()?;
    let timestamp = metadata
        .modified()
        .or_else(|_| metadata.created())
        .ok()?;
    let local_timestamp: chrono::DateTime<Local> = timestamp.into();
    Some(local_timestamp.naive_local().date())
}

fn extract_statement_end_from_filename(path: &Path) -> Option<NaiveDate> {
    let stem = path.file_stem()?.to_string_lossy();
    let last_token = stem.split('_').next_back()?;

    if last_token.len() != 8 || !last_token.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    NaiveDate::parse_from_str(last_token, "%Y%m%d").ok()
}

fn parse_yyyy_mm_dd(s: &str) -> Result<NaiveDate> {
    Ok(NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")?)
}

fn parse_flexible_date(s: &str) -> Result<NaiveDate> {
    let text = s.trim().trim_matches('"');

    if let Ok(date) = NaiveDate::parse_from_str(text, "%B %d, %Y") {
        return Ok(date);
    }
    if let Ok(date) = NaiveDate::parse_from_str(text, "%b %d, %Y") {
        return Ok(date);
    }
    if let Ok(date) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return Ok(date);
    }
    if let Ok(date) = NaiveDate::parse_from_str(text, "%m/%d/%Y") {
        return Ok(date);
    }

    Err(anyhow!("Unsupported date format: {}", s))
}

fn parse_ibkr_datetime_date(s: &str) -> Result<NaiveDate> {
    // "2025-12-06, 15:59:09 EST" or "2025-11-27, 15:41:12"
    let s = s.trim();
    let date_part = s.split(',').next().unwrap_or(s).trim();
    parse_yyyy_mm_dd(date_part)
}

fn parse_f64(s: &str) -> Result<f64> {
    let t = s.trim();
    if t.is_empty() || t == "--" {
        return Err(anyhow!("empty/NA number"));
    }
    let t = t.replace(',', "");
    Ok(t.parse::<f64>()?)
}

fn parse_f64_opt(s: &str) -> Option<f64> {
    parse_f64(s).ok()
}

fn cashflow_to_txn(checking_or_savings: &str, signed: f64) -> (String, String, String, f64) {
    // Database convention: amount is positive; direction carried by from/to + type.
    if signed >= 0.0 {
        (
            "income".to_string(),
            "EXTERNAL_PAYER".to_string(),
            checking_or_savings.to_string(),
            signed.abs(),
        )
    } else {
        (
            "expense".to_string(),
            checking_or_savings.to_string(),
            "EXTERNAL_PAYEE".to_string(),
            signed.abs(),
        )
    }
}

fn build_instrument_id(
    conid: &str,
    security_id: &str,
    asset_category: &str,
    symbol: &str,
) -> String {
    let sid = security_id.trim();
    if looks_like_isin(sid) {
        return format!("IBKR_{}", sid);
    }

    let conid = conid.trim();
    if !conid.is_empty() {
        return format!("IBKR_CONID_{}", conid);
    }

    let ac = asset_category.trim().replace(' ', "_");
    let sym = symbol.trim().replace(' ', "_");
    format!("IBKR_{}_{}", ac, sym)
}

fn split_symbol_aliases(symbol: &str) -> Vec<String> {
    let mut aliases: Vec<String> = Vec::new();
    for part in symbol.split(',') {
        let alias = part.trim();
        if !alias.is_empty() && !aliases.iter().any(|a| a == alias) {
            aliases.push(alias.to_string());
        }
    }
    aliases
}

fn primary_symbol(symbol: &str) -> String {
    split_symbol_aliases(symbol)
        .into_iter()
        .next()
        .unwrap_or_else(|| symbol.trim().to_string())
}

fn resolve_instrument_id(id: &str, remap: &HashMap<String, String>) -> String {
    let mut current = id.to_string();
    while let Some(next) = remap.get(&current) {
        if *next == current {
            break;
        }
        current = next.clone();
    }
    current
}

fn merge_instrument_entries(existing: &Value, incoming: &Value) -> Value {
    let existing_score = instrument_completeness_score(existing);
    let incoming_score = instrument_completeness_score(incoming);
    if incoming_score >= existing_score {
        incoming.clone()
    } else {
        existing.clone()
    }
}

fn instrument_completeness_score(inst: &Value) -> usize {
    ["description", "security_id", "type"]
    .iter()
    .filter(|field| match inst.get(**field) {
        Some(Value::Null) | None => false,
        Some(Value::String(s)) => !s.trim().is_empty(),
        Some(Value::Number(_)) => true,
        _ => true,
    })
    .count()
}

fn looks_like_isin(s: &str) -> bool {
    if s.len() != 12 {
        return false;
    }
    s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn build_txn_id(
    prefix: &str,
    date: NaiveDate,
    amount: f64,
    currency: &str,
    desc: &str,
    extra: &str,
) -> String {
    let key = format!(
        "{}|{}|{:.8}|{}|{}|{}",
        prefix,
        date.format("%Y-%m-%d"),
        amount,
        currency.trim(),
        desc.trim(),
        extra.trim()
    );
    let h = make_hash_id(&key);
    format!("{}-{}", prefix, &h[..24])
}

fn make_hash_id(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let hash = hasher.finalize();
    hex::encode(hash)
}
