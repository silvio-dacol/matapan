use anyhow::Result;
use std::{env, fs};

use skandinaviska_enskilda_banken::SebXlsxParser;

fn main() -> Result<()> {
    // Usage:
    //   seb_parser [file1.xlsx file2.xlsx ...] [database_path] [output_path]
    //
    // If no .xlsx files specified, will auto-discover all .xlsx files in current directory
    //
    // Defaults:
    //   Auto-discover all .xlsx files in current directory
    //   database_path: ../../../../database (resolves to database.json)
    //   output = same as database_path

    let args: Vec<String> = env::args().collect();

    // Separate .xlsx files from other arguments
    let mut xlsx_files: Vec<String> = Vec::new();
    let mut other_args: Vec<String> = Vec::new();
    
    for arg in args.iter().skip(1) {
        if arg.ends_with(".xlsx") {
            xlsx_files.push(arg.clone());
        } else {
            other_args.push(arg.clone());
        }
    }

    // If no .xlsx files specified, auto-discover them
    if xlsx_files.is_empty() {
        println!("📂 No .xlsx files specified, scanning current directory...");
        let current_dir = env::current_dir()?;
        for entry in fs::read_dir(&current_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("xlsx") {
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

    // Parse remaining arguments
    let database_path = other_args.get(0).map(|s| s.as_str()).unwrap_or("../../../../database");
    let output_path = other_args.get(1).map(|s| s.as_str());

    // Create parser with default account IDs
    let parser = SebXlsxParser::new("SEB_CHECKING", "SEB_SAVINGS");

    // Parse all discovered .xlsx files
    let mut all_txns = Vec::new();

    for (idx, xlsx_file) in xlsx_files.iter().enumerate() {
        // Determine account ID based on filename
        let account_id = if xlsx_file.to_lowercase().contains("saving") ||
                           xlsx_file.to_lowercase().contains("spar") {
            parser.account_id_savings.clone()
        } else if xlsx_file.to_lowercase().contains("check") ||
                  xlsx_file.to_lowercase().contains("transaction") {
            parser.account_id_checking.clone()
        } else {
            // Default: assign based on order or use generic naming
            format!("SEB_ACCOUNT_{}", idx + 1)
        };

        println!("\n📖 Parsing {} (account: {})", xlsx_file, account_id);
        match parser.parse_file(xlsx_file, &account_id) {
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
        eprintln!("❌ No transactions found in either file!");
        return Ok(());
    }

    // Create accounts
    let accounts = parser.create_accounts();
    let system_accounts = utils::create_system_accounts();

    // Read database.json (automatically initializes if missing or invalid)
    println!("📖 Reading database from: {}", database_path);
    let template = utils::read_database(database_path)?;

    // Merge system accounts first (EXTERNAL_PAYER, EXTERNAL_PAYEE, etc.)
    let (template_with_sys_accounts, sys_account_stats) = 
        skandinaviska_enskilda_banken::merge_accounts_into_template(template, system_accounts)?;

    // Then merge parser-specific accounts
    let (template_with_accounts, account_stats) = 
        skandinaviska_enskilda_banken::merge_accounts_into_template(template_with_sys_accounts, accounts)?;

    // Finally merge transactions with duplicate detection
    let (merged, txn_stats) = 
        skandinaviska_enskilda_banken::merge_transactions_into_template(template_with_accounts, all_txns)?;

    // Write to output path (defaults to database path)
    let final_output_path = output_path.unwrap_or(database_path);
    let written_path = utils::write_database(final_output_path, &merged)?;

    println!("\n📊 Summary:");
    println!("─────────────────────────────────────────");
    println!("✓ Processed {} system accounts: {} added, {} skipped (already exist)",
        sys_account_stats.total,
        sys_account_stats.added,
        sys_account_stats.skipped
    );
    println!("✓ Processed {} accounts: {} added, {} skipped (already exist)",
        account_stats.total,
        account_stats.added,
        account_stats.skipped
    );
    println!("✓ Processed {} transactions: {} added, {} skipped (duplicates)", 
        txn_stats.total,
        txn_stats.added,
        txn_stats.skipped
    );
    println!("✓ Total accounts in database: {}",
        merged.get("accounts")
            .and_then(|a| a.as_array())
            .map(|a| a.len())
            .unwrap_or(0)
    );    
    println!("✓ Total transactions in database: {}", 
        merged.get("transactions")
            .and_then(|t| t.as_array())
            .map(|a| a.len())
            .unwrap_or(0)
    );
    println!("─────────────────────────────────────────");
    println!("✅ Database written to: {}", written_path.display());
    
    Ok(())
}

