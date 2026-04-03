use anyhow::Result;
use std::{env, fs, path::PathBuf};

use ibkr_parser::{merge_instruments_with_deduplication, IbkrCsvParser};

fn find_csv_file() -> Option<PathBuf> {
    let current_dir = env::current_dir().ok()?;
    let entries = fs::read_dir(&current_dir).ok()?;
    
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("csv") {
            return Some(path);
        }
    }
    None
}

fn main() -> Result<()> {
    utils::load_dotenv();

    // Usage:
    //   ibkr_parser [csv_file] [database_path] [output_path]
    //
    // If csv_file is not provided, will automatically find the first .csv file in current directory
    // Defaults:
    //   database_path: ../../../../database
    //   output_path: same as database_path

    let args: Vec<String> = env::args().collect();
    
    let csv_path_string = if let Some(arg) = args.get(1) {
        arg.clone()
    } else if let Some(found) = find_csv_file() {
        found.to_string_lossy().to_string()
    } else {
        anyhow::bail!("No CSV file found in current directory. Please provide a CSV file path as the first argument.");
    };
    
    let csv_path = csv_path_string.as_str();
    let database_path = args
        .get(2)
        .map(|s| s.as_str())
        .unwrap_or("../../../../database");
    let output_path = args.get(3).map(|s| s.as_str());

    println!("📖 Parsing IBKR statement: {}", csv_path);

    let parser = IbkrCsvParser::new();
    let accounts = parser.create_accounts();

    let parsed = parser.parse_file(csv_path)?;

    if parsed.transactions.is_empty()
        && parsed.positions.is_empty()
        && parsed.instruments.is_empty()
    {
        println!("❌ Nothing parsed.");
        return Ok(());
    }

    println!(
        "✓ Parsed: {} txns, {} instruments, {} positions (as_of={})",
        parsed.transactions.len(),
        parsed.instruments.len(),
        parsed.positions.len(),
        parsed
            .statement_end
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );

    println!("📖 Reading database from: {}", database_path);
    let template = utils::read_database(database_path)?;

    let system_accounts = utils::create_system_accounts();
    let (db1, sys_stats) = utils::merge_accounts_with_deduplication(template, system_accounts)?;
    let (db2, acc_stats) = utils::merge_accounts_with_deduplication(db1, accounts)?;

    let (db3, inst_stats) = merge_instruments_with_deduplication(db2, parsed.instruments)?;
    let (db4, pos_stats) = utils::merge_positions_with_deduplication(db3, parsed.positions)?;

    let (mut merged, txn_stats) =
        utils::merge_transactions_with_deduplication(db4, parsed.transactions)?;

    let description_en_updated = utils::enrich_descriptions_to_english(&mut merged)?;
    let rules_changed = utils::apply_rules_from_database_path(&mut merged, database_path)?;

    let final_output_path = output_path.unwrap_or(database_path);
    let written = utils::write_database(final_output_path, &merged)?;

    println!("\n📊 Summary:");
    println!(
        "✓ System accounts processed: {} added, {} skipped",
        sys_stats.added, sys_stats.skipped
    );
    println!(
        "✓ Accounts processed: {} added, {} skipped",
        acc_stats.added, acc_stats.skipped
    );
    println!(
        "✓ Instruments processed: {} added, {} skipped",
        inst_stats.added, inst_stats.skipped
    );
    println!(
        "✓ Positions processed: {} added, {} skipped",
        pos_stats.added, pos_stats.skipped
    );
    println!(
        "✓ Transactions processed: {} added, {} skipped",
        txn_stats.added, txn_stats.skipped
    );
    println!(
        "✓ description-en updated: {} transaction(s)",
        description_en_updated
    );
    println!("✓ Rules changed: {} transaction(s)", rules_changed);
    println!("✅ Database written to: {}", written.display());

    // Rebuild the normalised database when FREECURRENCYAPI_KEY is available.
    if let Ok(api_key) = std::env::var("FREECURRENCYAPI_KEY") {
        println!("\n🔄 Syncing normalised database...");
        let db_dir = std::path::Path::new(final_output_path);
        match utils::sync_normalized_database_blocking(db_dir, &api_key) {
            Ok(()) => println!("✅ database_normalized.json updated."),
            Err(e) => eprintln!("⚠  FX sync failed (database_normalized.json not updated): {}", e),
        }
    }

    Ok(())
}
