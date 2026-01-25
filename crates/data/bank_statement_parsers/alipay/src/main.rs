use anyhow::{Context, Result};
use std::{
    env,
    fs::{self, File},
    io::Read,
};

use alipay::AlipayCsvParser;

fn main() -> Result<()> {
    // Usage:
    //   alipay_parser [file1.csv file2.csv ...] [database_path] [output_path]
    //
    // If no .csv files specified, will auto-discover all .csv files in current directory
    //
    // Defaults:
    //   Auto-discover all .csv files in current directory
    //   database_path: ../../../../database (resolves to database.json)
    //   output = same as database_path

    let args: Vec<String> = env::args().collect();

    // Separate .csv files from other arguments
    let mut csv_files: Vec<String> = Vec::new();
    let mut other_args: Vec<String> = Vec::new();

    for arg in args.iter().skip(1) {
        if arg.ends_with(".csv") {
            csv_files.push(arg.clone());
        } else {
            other_args.push(arg.clone());
        }
    }

    // If no .csv files specified, auto-discover them
    if csv_files.is_empty() {
        println!("📂 No .csv files specified, scanning current directory...");
        let current_dir = env::current_dir()?;
        for entry in fs::read_dir(&current_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("csv") {
                if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                    csv_files.push(filename.to_string());
                    println!("  ✓ Found: {}", filename);
                }
            }
        }
    }

    if csv_files.is_empty() {
        eprintln!("❌ No .csv files found!");
        return Ok(());
    }

    // Parse remaining arguments
    let database_path = other_args
        .get(0)
        .map(|s| s.as_str())
        .unwrap_or("../../../../database");
    let output_path = other_args.get(1).map(|s| s.as_str());

    let account_id = "ALIPAY_WALLET".to_string();
    let parser = AlipayCsvParser::new(&account_id);

    // Parse all discovered .csv files
    let mut all_txns = Vec::new();

    for csv_file_path in &csv_files {
        println!("\n📖 Parsing {} (account: {})", csv_file_path, account_id);

        let mut csv_file =
            File::open(csv_file_path).with_context(|| format!("Cannot open {}", csv_file_path))?;
        let mut csv_buf = Vec::new();
        csv_file.read_to_end(&mut csv_buf)?;

        match parser.parse_reader(csv_buf.as_slice()) {
            Ok(txns) => {
                println!("  ✓ Found {} transactions", txns.len());
                all_txns.extend(txns);
            }
            Err(e) => {
                eprintln!("  ⚠ Warning: Could not parse file: {}", e);
                eprintln!("    Continuing with next file...");
            }
        }
    }

    if all_txns.is_empty() {
        eprintln!("❌ No transactions found in any file!");
        return Ok(());
    }

    let accounts = parser.create_accounts();
    let system_accounts = utils::create_system_accounts();

    // Read database.json (utils handles initialization if needed)
    println!("📖 Reading database from: {}", database_path);
    let template = utils::read_database(database_path)?;

    // Merge system accounts first (EXTERNAL_PAYER, EXTERNAL_PAYEE, etc.)
    let (db1, sys_stats) = alipay::merge_accounts_into_template(template, system_accounts)?;

    // Merge parser accounts
    let (db2, acc_stats) = alipay::merge_accounts_into_template(db1, accounts)?;

    // Merge transactions
    let (merged, txn_stats) = alipay::merge_transactions_into_template(db2, all_txns)?;

    // Write
    let final_output_path = output_path.unwrap_or(database_path);
    let written = utils::write_database(final_output_path, &merged)?;

    println!("\n📊 Summary:");
    println!("─────────────────────────────────────────");
    println!(
        "✓ Processed {} system accounts: {} added, {} skipped (already exist)",
        sys_stats.total, sys_stats.added, sys_stats.skipped
    );
    println!(
        "✓ Processed {} accounts: {} added, {} skipped (already exist)",
        acc_stats.total, acc_stats.added, acc_stats.skipped
    );
    println!(
        "✓ Processed {} transactions: {} added, {} skipped (duplicates)",
        txn_stats.total, txn_stats.added, txn_stats.skipped
    );
    println!(
        "✓ Total accounts in database: {}",
        merged
            .get("accounts")
            .and_then(|a| a.as_array())
            .map(|a| a.len())
            .unwrap_or(0)
    );
    println!(
        "✓ Total transactions in database: {}",
        merged
            .get("transactions")
            .and_then(|t| t.as_array())
            .map(|a| a.len())
            .unwrap_or(0)
    );
    println!("─────────────────────────────────────────");
    println!("✅ Database written to: {}", written.display());

    Ok(())
}
