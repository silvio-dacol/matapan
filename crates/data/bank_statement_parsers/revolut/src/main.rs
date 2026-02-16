use anyhow::{Context, Result};
use std::{
    env,
    fs::{self, File},
    io::Read,
};

use revolut::RevolutCsvParser;

fn main() -> Result<()> {
    // Usage:
    //   revolut_parser [file1.csv file2.csv ...] [database_path] [output_path]
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
    let database_path = other_args.get(0).map(|s| s.as_str()).unwrap_or("../../../../database");
    let output_path = other_args.get(1).map(|s| s.as_str());

    // Parse all discovered .csv files
    let mut all_txns = Vec::new();
    let mut all_used_account_ids = std::collections::HashSet::new();

    for csv_file_path in csv_files.iter() {
        println!("\n📖 Parsing {} (account base: REVOLUT)", csv_file_path);
        
        // Read CSV
        let mut csv_file = File::open(csv_file_path)
            .with_context(|| format!("Cannot open {}", csv_file_path))?;
        let mut csv_buf = Vec::new();
        csv_file.read_to_end(&mut csv_buf)?;

        // Parse
        let parser = RevolutCsvParser::new("REVOLUT");
        match parser.parse_reader(csv_buf.as_slice()) {
            Ok((txns, used_accounts)) => {
                println!("  ✓ Found {} transactions", txns.len());
                all_txns.extend(txns);
                all_used_account_ids.extend(used_accounts);
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

    // Create only the accounts that were actually used
    let parser = RevolutCsvParser::new("REVOLUT"); // Base name doesn't matter here
    let used_account_ids: Vec<String> = all_used_account_ids.into_iter().collect();
    let all_accounts = parser.create_used_accounts(&used_account_ids);
    let system_accounts = utils::create_system_accounts();

    // Read database.json (automatically initializes if missing or invalid)
    println!("\n📖 Reading database from: {}", database_path);
    let template = utils::read_database(database_path)?;

    // Merge system accounts first (EXTERNAL_PAYER, EXTERNAL_PAYEE, etc.)
    let (template_with_sys_accounts, sys_account_stats) = 
        revolut::merge_accounts_into_template(template, system_accounts)?;

    // Then merge parser-specific accounts
    let (template_with_accounts, account_stats) = 
        revolut::merge_accounts_into_template(template_with_sys_accounts, all_accounts)?;

    // Finally merge transactions with duplicate detection
    let (merged, txn_stats) = 
        revolut::merge_transactions_into_template(template_with_accounts, all_txns)?;

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
    );    println!("✓ Total transactions in database: {}", 
        merged.get("transactions")
            .and_then(|t| t.as_array())
            .map(|a| a.len())
            .unwrap_or(0)
    );
    println!("─────────────────────────────────────────");
    println!("✅ Database written to: {}", written_path.display());
    
    Ok(())
}
