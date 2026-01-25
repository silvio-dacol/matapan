use anyhow::{Context, Result};
use std::{env, fs};

use ccb_parser::CcbXlsParser;

fn main() -> Result<()> {
    // Usage:
    //   ccb_parser [file1.xls file2.xls ...] [database_path] [output_path]
    //
    // If no .xls files specified, will auto-discover ccb*.xls in current directory
    //
    // Defaults:
    //   Auto-discover ccb*.xls in current directory
    //   database_path: ../../../../database (resolves to database.json)
    //   output = same as database_path

    let args: Vec<String> = env::args().collect();

    // Separate .xls files from other arguments
    let mut xls_files: Vec<String> = Vec::new();
    let mut other_args: Vec<String> = Vec::new();

    for arg in args.iter().skip(1) {
        if arg.to_lowercase().ends_with(".xls") {
            xls_files.push(arg.clone());
        } else {
            other_args.push(arg.clone());
        }
    }

    // If no .xls files specified, auto-discover ccb*.xls
    if xls_files.is_empty() {
        println!("📂 No .xls files specified, scanning current directory...");
        for entry in fs::read_dir(".").context("Cannot read current directory")? {
            let path = entry?.path();
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                let low = name.to_lowercase();
                if low.starts_with("ccb") && low.ends_with(".xls") {
                    xls_files.push(name.to_string());
                    println!("  ✓ Found: {}", name);
                }
            }
        }
        xls_files.sort();
    }

    if xls_files.is_empty() {
        println!("❌ No .xls input files found (expected ccb*.xls or explicit paths).");
        return Ok(());
    }

    // Parse remaining arguments
    let database_path = other_args
        .get(0)
        .map(|s| s.as_str())
        .unwrap_or("../../../../database");
    let output_path = other_args.get(1).map(|s| s.as_str());

    let account_id = "CCB_CHECKING".to_string();
    let parser = CcbXlsParser::new(&account_id);

    println!(
        "📖 Parsing {} file(s) for account {}",
        xls_files.len(),
        account_id
    );

    let accounts = parser.create_accounts();
    let mut all_txns = Vec::new();

    for p in &xls_files {
        println!("  • {}", p);
        let txns = parser
            .parse_file(p)
            .with_context(|| format!("Failed parsing {}", p))?;
        all_txns.extend(txns);
    }

    if all_txns.is_empty() {
        println!("❌ No transactions found.");
        return Ok(());
    }

    println!("📖 Reading database from: {}", database_path);
    let template = utils::read_database(database_path)?;

    let system_accounts = utils::create_system_accounts();
    let (db1, sys_stats) = ccb_parser::merge_accounts_into_template(template, system_accounts)?;
    let (db2, acc_stats) = ccb_parser::merge_accounts_into_template(db1, accounts)?;
    let (merged, txn_stats) = ccb_parser::merge_transactions_into_template(db2, all_txns)?;

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
