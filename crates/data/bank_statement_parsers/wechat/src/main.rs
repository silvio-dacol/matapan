use anyhow::{Context, Result};
use std::{env, fs};

use wechat::WeChatXlsxParser;

fn main() -> Result<()> {
    // Usage:
    //   wechat [file1.xlsx file2.xlsx ...] [database_path] [output_path]
    //
    // If no .xlsx files specified, will auto-discover all wechat_*.xlsx in current directory
    //
    // Defaults:
    //   Auto-discover wechat_*.xlsx in current directory
    //   database_path: ../../../../database (resolves to database.json)
    //   output = same as database_path

    let args: Vec<String> = env::args().collect();

    // Separate .xlsx files from other arguments
    let mut xlsx_files: Vec<String> = Vec::new();
    let mut other_args: Vec<String> = Vec::new();

    for arg in args.iter().skip(1) {
        if arg.to_lowercase().ends_with(".xlsx") {
            xlsx_files.push(arg.clone());
        } else {
            other_args.push(arg.clone());
        }
    }

    // If no .xlsx files specified, auto-discover wechat_*.xlsx
    if xlsx_files.is_empty() {
        println!("📂 No .xlsx files specified, scanning current directory...");
        for entry in fs::read_dir(".").context("Cannot read current directory")? {
            let path = entry?.path();
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name.to_lowercase().starts_with("wechat_")
                    && name.to_lowercase().ends_with(".xlsx")
                {
                    xlsx_files.push(name.to_string());
                    println!("  ✓ Found: {}", name);
                }
            }
        }
        xlsx_files.sort();
    }

    if xlsx_files.is_empty() {
        println!("❌ No .xlsx input files found (expected wechat_*.xlsx or explicit paths).");
        return Ok(());
    }

    // Parse remaining arguments
    let database_path = other_args
        .get(0)
        .map(|s| s.as_str())
        .unwrap_or("../../../../database");
    let output_path = other_args.get(1).map(|s| s.as_str());

    let account_id = "WECHAT_WALLET".to_string();
    let parser = WeChatXlsxParser::new(&account_id);

    println!(
        "📖 Parsing {} file(s) for account {}",
        xlsx_files.len(),
        account_id
    );

    let all_accounts = parser.create_accounts();
    let mut all_txns = Vec::new();

    for p in &xlsx_files {
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
    let (db1, sys_stats) = wechat::merge_accounts_into_template(template, system_accounts)?;
    let (db2, acc_stats) = wechat::merge_accounts_into_template(db1, all_accounts)?;
    let (merged, txn_stats) = wechat::merge_transactions_into_template(db2, all_txns)?;

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
