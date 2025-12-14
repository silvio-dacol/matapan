use calamine::{open_workbook_auto, Data, Reader};
use chrono::NaiveDate;
use serde::Serialize;
use std::{error::Error, path::Path};

#[derive(Debug, Serialize)]
pub struct EngineTransaction {
    pub txn_id: String,
    pub date: String,       // YYYY-MM-DD
    pub account_id: String, // e.g. "INTESA_CURRENT"
    #[serde(rename = "type")]
    pub txn_type: String, // income | expense | internal_transfer | other
    pub category: String,   // normalized category
    pub amount: f64,
    pub currency: String,
    pub description: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,
}

pub fn parse_intesa_sanpaolo_xlsx<P: AsRef<Path>>(
    path: P,
    account_id: &str,
) -> Result<Vec<EngineTransaction>, Box<dyn Error>> {
    let mut wb = open_workbook_auto(path)?;
    let mut out: Vec<EngineTransaction> = Vec::new();

    for sheet_name in wb.sheet_names().to_owned() {
        let range = match wb.worksheet_range(&sheet_name) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let rows: Vec<Vec<Data>> = range.rows().map(|r| r.to_vec()).collect();
        if rows.is_empty() {
            continue;
        }

        // Find header row (Intesa export has lots of intro text above)
        let mut header_idx: Option<usize> = None;
        let mut headers: Vec<String> = Vec::new();
        for (i, row) in rows.iter().enumerate() {
            let hdr: Vec<String> = row.iter().map(cell_str).collect();
            if looks_like_intesa_headers(&hdr) {
                header_idx = Some(i);
                headers = hdr;
                break;
            }
        }
        let Some(hidx) = header_idx else { continue };

        // Column indices (your file usually has: Data, Operazione, Dettagli, Categoria, Valuta, Importo)
        let mut idx_date: Option<usize> = None;
        let mut idx_op: Option<usize> = None;
        let mut idx_details: Option<usize> = None;
        let mut idx_category: Option<usize> = None;
        let mut idx_currency: Option<usize> = None;
        let mut idx_amount: Option<usize> = None;

        for (i, h) in headers.iter().enumerate() {
            let hl = h.trim().to_lowercase();
            if idx_date.is_none() && hl == "data" {
                idx_date = Some(i);
            }
            if idx_op.is_none() && hl.contains("operazione") {
                idx_op = Some(i);
            }
            if idx_details.is_none() && hl.contains("dettagli") {
                idx_details = Some(i);
            }
            if idx_category.is_none() && hl.contains("categoria") {
                idx_category = Some(i);
            }
            if idx_currency.is_none() && hl.contains("valuta") {
                idx_currency = Some(i);
            }
            if idx_amount.is_none() && hl.contains("importo") {
                idx_amount = Some(i);
            }
        }

        // Parse rows below header
        for (rid, row) in rows.iter().enumerate().skip(hidx + 1) {
            let date_cell = idx_date.and_then(|i| row.get(i));
            let op = idx_op
                .and_then(|i| row.get(i))
                .map(cell_str)
                .unwrap_or_default();
            let details = idx_details
                .and_then(|i| row.get(i))
                .map(cell_str)
                .unwrap_or_default();
            let category_raw = idx_category
                .and_then(|i| row.get(i))
                .map(cell_str)
                .unwrap_or_default();
            let currency = idx_currency
                .and_then(|i| row.get(i))
                .map(cell_str)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "EUR".to_string());

            let amount_cell = idx_amount.and_then(|i| row.get(i));
            let amount = amount_cell
                .and_then(cell_f64)
                .or_else(|| amount_cell.map(cell_str).as_deref().and_then(parse_amount))
                .unwrap_or(0.0);

            // stop/skip empty rows
            if op.is_empty() && details.is_empty() && amount == 0.0 {
                continue;
            }

            let date = date_cell
                .and_then(parse_excelish_date)
                .unwrap_or_else(|| "1970-01-01".to_string());

            let description = join_description(&op, &details);

            // type + category
            let (txn_type, category) = if !category_raw.is_empty() {
                infer_from_intesa_category(&category_raw, amount)
            } else {
                infer_from_description(&description, amount)
            };

            let txn_id = format!("{}-{}-INTESA-{}", date, account_id, rid);

            out.push(EngineTransaction {
                txn_id,
                date,
                account_id: account_id.to_string(),
                txn_type,
                category,
                amount,
                currency,
                description,
                tags: Vec::new(),
            });
        }
    }

    Ok(out)
}

fn looks_like_intesa_headers(headers: &[String]) -> bool {
    let h = headers
        .iter()
        .map(|s| s.trim().to_lowercase())
        .collect::<Vec<_>>();
    let has_data = h.iter().any(|x| x == "data");
    let has_importo = h.iter().any(|x| x.contains("importo"));
    let has_valuta = h.iter().any(|x| x.contains("valuta"));
    let has_operazione_or_dettagli =
        h.iter().any(|x| x.contains("operazione")) || h.iter().any(|x| x.contains("dettagli"));
    has_data && has_importo && (has_valuta || has_operazione_or_dettagli)
}

fn join_description(op: &str, details: &str) -> String {
    let op = op.trim();
    let details = details.trim();
    if op.is_empty() {
        return details.to_string();
    }
    if details.is_empty() {
        return op.to_string();
    }
    format!("{} | {}", op, details)
}

fn infer_from_intesa_category(cat: &str, amount: f64) -> (String, String) {
    let c = cat.trim().to_lowercase();

    // If category says something “income-like”, override sign inference.
    if c.contains("stipendi") || c.contains("entrate") {
        return ("income".to_string(), "salary_main_job".to_string());
    }
    if c.contains("interessi") || c.contains("cedole") {
        return ("income".to_string(), "interest_dividend".to_string());
    }
    if c.contains("imposte") || c.contains("bolli") || c.contains("commissioni") {
        return ("expense".to_string(), "fees_taxes".to_string());
    }

    // Otherwise infer type from sign, but keep the bank category as normalized-ish
    let txn_type = if amount >= 0.0 { "income" } else { "expense" }.to_string();
    let category = normalize_category(&c);
    (txn_type, category)
}

fn infer_from_description(desc: &str, amount: f64) -> (String, String) {
    let d = desc.to_lowercase();
    if d.contains("bonifico") {
        return ("internal_transfer".to_string(), "transfer".to_string());
    }
    if d.contains("pos") || d.contains("carta") {
        return ("expense".to_string(), "card_payment".to_string());
    }
    if d.contains("stipendio") {
        return ("income".to_string(), "salary_main_job".to_string());
    }
    if d.contains("cedole") || d.contains("dividend") || d.contains("interessi") {
        return ("income".to_string(), "interest_dividend".to_string());
    }

    // fallback by sign
    let txn_type = if amount >= 0.0 { "income" } else { "expense" }.to_string();
    (txn_type, "intesa_other".to_string())
}

fn normalize_category(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn cell_str(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.trim().to_string(),
        Data::Float(f) => format!("{}", f),
        Data::Int(i) => format!("{}", i),
        Data::Bool(b) => {
            if *b {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Data::Error(_) => String::new(),
        Data::DateTime(f) => format!("{}", f),
        Data::DateTimeIso(s) => s.trim().to_string(),
        Data::DurationIso(s) => s.trim().to_string(),
    }
}

fn parse_amount(raw: &str) -> Option<f64> {
    // Italian format: thousands '.' and decimals ','
    let s = raw.trim().replace('.', "").replace(',', ".");
    s.parse::<f64>().ok()
}

fn cell_f64(cell: &Data) -> Option<f64> {
    match cell {
        Data::Float(f) => Some(*f),
        Data::Int(i) => Some(*i as f64),
        Data::String(s) => parse_amount(s),
        _ => None,
    }
}

fn parse_excelish_date(cell: &Data) -> Option<String> {
    // Intesa xlsx often stores as real Excel date OR ISO OR a string date
    match cell {
        // ExcelDateTime is not a plain serial; leave unparsed here
        Data::DateTime(_f) => None,
        Data::Float(f) => excel_serial_to_yyyy_mm_dd(*f),
        Data::Int(i) => excel_serial_to_yyyy_mm_dd(*i as f64),
        Data::DateTimeIso(s) => parse_date_string(s),
        Data::String(s) => parse_date_string(s),
        _ => None,
    }
}

fn parse_date_string(s: &str) -> Option<String> {
    let t = s.trim();
    if let Ok(d) = NaiveDate::parse_from_str(t, "%Y-%m-%d") {
        return Some(d.format("%Y-%m-%d").to_string());
    }
    if let Ok(d) = NaiveDate::parse_from_str(t, "%d/%m/%Y") {
        return Some(d.format("%Y-%m-%d").to_string());
    }
    None
}

fn excel_serial_to_yyyy_mm_dd(v: f64) -> Option<String> {
    // Excel epoch 1899-12-30 (accounts for 1900 leap-year bug)
    let base = NaiveDate::from_ymd_opt(1899, 12, 30)?;
    let days = v.floor() as i64;
    let d = base.checked_add_days(chrono::Days::new(days as u64))?;
    Some(d.format("%Y-%m-%d").to_string())
}

use std::fs;

fn main() -> Result<(), Box<dyn Error>> {
    // Usage:
    //   intesa_sanpaolo <input_xlsx> [output_json]
    // Defaults to dashboard/dashboard.json; use '-' to print to stdout.
    let args: Vec<String> = std::env::args().collect();

    let input_path = if args.len() > 1 {
        &args[1]
    } else {
        "intesa_sanpaolo.xlsx"
    };

    let default_output = "../../../../database/dashboard.json";
    let output_path = if args.len() > 2 {
        &args[2]
    } else {
        default_output
    };

    let account_id = "INTESA_CURRENT";
    let txs = parse_intesa_sanpaolo_xlsx(input_path, account_id)?;

    if output_path == "-" {
        println!("{}", serde_json::to_string_pretty(&txs)?);
    } else {
        let out_path = std::path::Path::new(output_path);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let merged = if out_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|n| n.eq_ignore_ascii_case("dashboard.json"))
            .unwrap_or(false)
        {
            merge_into_dashboard(out_path, &txs)?
        } else {
            serde_json::to_string_pretty(&txs)?
        };

        fs::write(out_path, merged)?;
        eprintln!("Wrote Intesa transactions to {}", out_path.display());
    }

    Ok(())
}

fn merge_into_dashboard(
    path: &std::path::Path,
    txs: &[EngineTransaction],
) -> Result<String, Box<dyn Error>> {
    // Read existing dashboard JSON if present; otherwise initialize with minimal structure.
    let existing = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        String::from("{\"transactions\":[]}")
    };

    let mut root: serde_json::Value =
        serde_json::from_str(&existing).unwrap_or_else(|_| serde_json::json!({"transactions": []}));

    if !root.get("transactions").is_some() || !root["transactions"].is_array() {
        root["transactions"] = serde_json::Value::Array(vec![]);
    }
    let arr = root["transactions"].as_array_mut().unwrap();

    for t in txs {
        arr.push(serde_json::to_value(t)?);
    }

    Ok(serde_json::to_string_pretty(&root)?)
}
