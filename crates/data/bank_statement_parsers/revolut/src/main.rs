use anyhow::{Context, Result};
use std::{
    env,
    fs::File,
    io::Read,
};

use revolut::RevolutCsvParser;

fn main() -> Result<()> {
    // Usage:
    //   revolut_parser <revolut.csv> <database_path> [output_path] [account_id]
    //
    // Defaults:
    //   revolut.csv
    //   ../../../../database (resolves to database.json)
    //   output = same as database_path
    //   account_id = REVOLUT_CURRENT

    let args: Vec<String> = env::args().collect();

    let input_csv = args.get(1).map(|s| s.as_str()).unwrap_or("revolut.csv");

    // Arg2 can be either a JSON file path or a folder path.
    let database_path = args.get(2).map(|s| s.as_str()).unwrap_or("../../../../database");
    
    let output_path = args.get(3).map(|s| s.as_str());
    let account_id = args
        .get(4)
        .cloned()
        .unwrap_or_else(|| "REVOLUT_CURRENT".to_string());

    // Read CSV
    let mut csv_file =
        File::open(input_csv).with_context(|| format!("Cannot open {}", input_csv))?;
    let mut csv_buf = Vec::new();
    csv_file.read_to_end(&mut csv_buf)?;

    // Parse
    let parser = RevolutCsvParser::new(account_id);
    let txns = parser.parse_reader(csv_buf.as_slice())?;
    let accounts = parser.create_accounts();
    let system_accounts = utils::create_system_accounts();

    // Read database.json (automatically initializes if missing or invalid)
    let template = utils::read_database(database_path)?;

    // Merge system accounts first (EXTERNAL_PAYER, EXTERNAL_PAYEE, etc.)
    let (template_with_sys_accounts, sys_account_stats) = 
        revolut::merge_accounts_into_template(template, system_accounts)?;

    // Then merge parser-specific accounts
    let (template_with_accounts, account_stats) = 
        revolut::merge_accounts_into_template(template_with_sys_accounts, accounts)?;

    // Finally merge transactions with duplicate detection
    let (merged, txn_stats) = 
        revolut::merge_transactions_into_template(template_with_accounts, txns)?;

    // Write to output path (defaults to database path)
    let final_output_path = output_path.unwrap_or(database_path);
    let written_path = utils::write_database(final_output_path, &merged)?;

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
    println!("✓ Database written to: {}", written_path.display());
    
    Ok(())
}
