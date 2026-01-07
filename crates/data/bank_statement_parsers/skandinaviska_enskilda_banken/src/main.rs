use anyhow::Result;
use std::{env, fs};

use seb::SebXlsxParser;

fn main() -> Result<()> {
    // Usage:
    //   seb_parser [file1.xlsx file2.xlsx ...] [database_path] [output_path]
    //
    // If no .xlsx files specified, will auto-discover all .xlsx files in current directory
    //
    // Defaults:
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

    if xlsx_files.is_empty() {
        println!("📂 No .xlsx files specified, scanning current directory...");
        let current_dir = env::current_dir()?;
        for entry in fs::read_dir(&current_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()).map(|s| s.eq_ignore_ascii_case("xlsx")) == Some(true) {
                if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                    xlsx_files.push(filename.to_string());
                    println!("  ✓ Found: {}", filename);
                }
            }
        }
    }

    if xlsx_files.is_empty() {
        eprintln!("❌ No .xlsx files found!");
        return Ok(());
    }

    let database_path = other_args.get(0).map(|s| s.as_str()).unwrap_or("../../../../database");
    let output_path = other_args.get(1).map(|s| s.as_str());

    // Your canonical account IDs (keep stable)
    let checking_id = "SEB_CHECKING".to_string();
    let savings_id = "SEB_SAVINGS".to_string();

    // Optional: digits-only account numbers for better internal transfer mapping
    // From your files these are:
    // Checking: 5020 01 052 05 -> "50200105205"
    // Savings:  5037 18 077 86 -> "50371807786"
    let parser = SebXlsxParser::new(checking_id.clone(), savings_id.clone())
        .with_account_numbers(Some("50200105205".to_string()), Some("50371807786".to_string()));

    let mut all_txns = Vec::new();

    for xlsx_path in &xlsx_files {
        let lower = xlsx_path.to_lowercase();

        let account_id = if lower.contains("saving") || lower.contains("savings") || lower.contains("spark") {
            &savings_id
        } else if lower.contains("check") || lower.contains("current") || lower.contains("privat") {
            &checking_id
        } else {
            // Fallback: guess based on smaller file name patterns
            // Default to checking so you don’t silently mis-route salary etc.
            &checking_id
        };

        println!("\n📖 Parsing {} (account: {})", xlsx_path, account_id);

        match parser.parse_file(xlsx_path, account_id) {
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

    // Accounts to insert
    let all_accounts = parser.create_accounts();
    let system_accounts = utils::create_system_accounts();

    println!("\n📖 Reading database from: {}", database_path);
    let template = utils::read_database(database_path)?;

    // Merge system accounts first
    let (template_with_sys, sys_stats) = seb::merge_accounts_into_template(template, system_accounts)?;

    // Then merge SEB accounts
    let (template_with_accounts, acc_stats) = seb::merge_accounts_into_template(template_with_sys, all_accounts)?;

    // Then merge transactions (dedupe by txn_id)
    let (merged, txn_stats) = seb::merge_transactions_into_template(template_with_accounts, all_txns)?;

    let final_output_path = output_path.unwrap_or(database_path);
    let written_path = utils::write_database(final_output_path, &merged)?;

    println!("\n📊 Summary:");
    println!("─────────────────────────────────────────");
    println!("✓ Processed {} system accounts: {} added, {} skipped",
        sys_stats.total, sys_stats.added, sys_stats.skipped
    );
    println!("✓ Processed {} accounts: {} added, {} skipped",
        acc_stats.total, acc_stats.added, acc_stats.skipped
    );
    println!("✓ Processed {} transactions: {} added, {} skipped (duplicates)",
        txn_stats.total, txn_stats.added, txn_stats.skipped
    );
    println!("✓ Total accounts in database: {}",
        merged.get("accounts").and_then(|a| a.as_array()).map(|a| a.len()).unwrap_or(0)
    );
    println!("✓ Total transactions in database: {}",
        merged.get("transactions").and_then(|t| t.as_array()).map(|a| a.len()).unwrap_or(0)
    );
    println!("─────────────────────────────────────────");
    println!("✅ Database written to: {}", written_path.display());

    Ok(())
}
