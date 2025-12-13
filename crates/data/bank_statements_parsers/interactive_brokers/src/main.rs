use csv::ReaderBuilder;
use serde_json::{Value, json};
use std::error::Error;
use std::fs;
use std::path::Path;

// Parse IBKR CSV focusing on the Trades section and emit template-compatible trade transactions
fn parse_ibkr_trades<P: AsRef<Path>>(path: P) -> Result<Vec<Value>, Box<dyn Error>> {
    let mut rdr = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(path)?;

    let mut in_trades_section = false;
    let mut txs: Vec<Value> = Vec::new();

    for result in rdr.records() {
        let rec = match result {
            Ok(r) => r,
            Err(_) => continue,
        };
        if rec.len() == 0 {
            continue;
        }
        // Section detection
        if rec.get(0) == Some("Trades") && rec.get(1) == Some("Header") {
            in_trades_section = true;
            continue;
        }
        // Stop when a new header of another section encountered
        if in_trades_section && rec.get(1) == Some("Header") && rec.get(0) != Some("Trades") {
            in_trades_section = false;
        }
        if !in_trades_section {
            continue;
        }
        // Parse trades data lines: Trades,Data,Order,Asset Category,Currency,Symbol,Date/Time,Quantity,T. Price,C. Price,Proceeds,Comm/Fee,Basis,Realized P/L,MTM P/L,Code,
        if rec.get(0) == Some("Trades") && rec.get(1) == Some("Data") {
            let asset_category = rec.get(3).unwrap_or("");
            let currency = rec.get(4).unwrap_or("");
            let symbol = rec.get(5).unwrap_or("");
            let date_time = rec.get(6).unwrap_or("");
            let quantity = rec.get(7).unwrap_or("0");
            let t_price = rec.get(8).unwrap_or("0");
            let proceeds = rec.get(10).unwrap_or("0");
            let comm_fee = rec.get(11).unwrap_or("0");

            // Normalize date: input like "2025-01-06, 11:24:47" -> "2025-01-06"
            let date = if let Some((d, _)) = date_time.split_once(',') {
                d.trim().to_string()
            } else {
                date_time.to_string()
            };

            // Determine buy/sell by proceeds sign: purchases typically negative proceeds
            let proceeds_val: f64 = proceeds.replace(',', "").parse().unwrap_or(0.0);
            let qty_val: f64 = quantity.replace(',', "").parse().unwrap_or(0.0);
            let price_val: f64 = t_price.replace(',', "").parse().unwrap_or(0.0);
            let fees_val: f64 = comm_fee.replace(',', "").parse().unwrap_or(0.0);

            let category = if proceeds_val < 0.0 {
                "investment_buy"
            } else {
                "investment_sell"
            };

            // Build transaction in template format (type: "trade")
            let txn_id = format!("{}-IBKR-{}", date, symbol);
            let tx = json!({
                "txn_id": txn_id,
                "date": date,
                "account_id": "IBKR_MARGIN",
                "type": "trade",
                "category": category,
                "instrument_id": symbol,
                "quantity": qty_val,
                "price_per_unit": price_val,
                "gross_amount": proceeds_val, // IBKR Proceeds field
                "currency": currency,
                "fees": fees_val,
                "description": format!("{} trade {} {}", asset_category, symbol, qty_val),
                "tags": ["investment"]
            });

            txs.push(tx);
        }
    }

    Ok(txs)
}

fn merge_into_dashboard(
    out_path: &std::path::Path,
    new_txs: &[Value],
) -> Result<String, Box<dyn Error>> {
    let existing: Option<Value> = if out_path.exists() {
        let content = fs::read_to_string(out_path)?;
        match serde_json::from_str::<Value>(&content) {
            Ok(v) => Some(v),
            Err(_) => None,
        }
    } else {
        None
    };

    let mut root = existing.unwrap_or_else(|| Value::Object(serde_json::Map::new()));

    let mut txs_array: Vec<Value> = match root.get("transactions") {
        Some(v) => v.as_array().cloned().unwrap_or_else(|| Vec::new()),
        None => Vec::new(),
    };

    use std::collections::HashSet;
    let mut existing_ids: HashSet<String> = HashSet::new();
    for v in txs_array.iter() {
        if let Some(id) = v.get("txn_id").and_then(|x| x.as_str()) {
            existing_ids.insert(id.to_string());
        }
    }

    for tx in new_txs {
        if let Some(id) = tx.get("txn_id").and_then(|x| x.as_str()) {
            if existing_ids.contains(id) {
                continue;
            }
        }
        txs_array.push(tx.clone());
    }

    txs_array.sort_by(|a, b| {
        let ad = a.get("date").and_then(|x| x.as_str()).unwrap_or("");
        let bd = b.get("date").and_then(|x| x.as_str()).unwrap_or("");
        ad.cmp(bd)
    });

    root.as_object_mut()
        .unwrap()
        .insert("transactions".to_string(), Value::Array(txs_array));

    Ok(serde_json::to_string_pretty(&root)?)
}

fn main() -> Result<(), Box<dyn Error>> {
    // Usage:
    //   interactive_brokers <input_csv> [output_json]
    // Defaults to dashboard/dashboard.json; use '-' to print to stdout.
    let args: Vec<String> = std::env::args().collect();

    let input_path = if args.len() > 1 {
        &args[1]
    } else {
        "interactive_brokers.csv"
    };
    let default_output = "../../../../database/dashboard.json";
    let output_path = if args.len() > 2 {
        &args[2]
    } else {
        default_output
    };

    let txs = parse_ibkr_trades(input_path)?;

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
        eprintln!("Wrote IBKR transactions to {}", out_path.display());
    }

    Ok(())
}
