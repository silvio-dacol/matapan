use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    env,
    fs::File,
    io::Read,
};

use revolut::RevolutCsvParser;

fn main() -> Result<()> {
    // Usage:
    //   revolut_parser [database_path] [output_path]
    //
    // Auto-discovers all .csv files in current directory.
    //
    // Defaults:
    //   Auto-discover all .csv files in current directory
    //   database_path: ../../../../database (resolves to database.json)
    //   output = same as database_path

    let args: Vec<String> = env::args().collect();
    let csv_files = utils::discover_input_files_in_current_dir(&[utils::InputFormat::Csv])?;

    if csv_files.is_empty() {
        eprintln!("❌ No .csv files found!");
        return Ok(());
    }

    println!("📂 Input files:");
    for file in &csv_files {
        println!("  ✓ Found: {}", file);
    }

    // Parse arguments (no per-file overrides; files are always discovered from current directory)
    let database_path = args.get(1).map(|s| s.as_str()).unwrap_or("../../../../database");
    let output_path = args.get(2).map(|s| s.as_str());

    // Parse all discovered .csv files
    let mut all_txns = Vec::new();
    let mut all_used_account_ids = std::collections::HashSet::new();

    utils::for_each_input_file(&csv_files, |csv_file_path| {
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

        Ok(())
    })?;

    // Create only the accounts that were actually used
    let parser = RevolutCsvParser::new("REVOLUT"); // Base name doesn't matter here
    let used_account_ids: Vec<String> = all_used_account_ids.into_iter().collect();
    let all_accounts = parser.create_used_accounts(&used_account_ids);

    let parsed_entities = utils::ParsedEntities {
        accounts: all_accounts,
        transactions: all_txns,
        ..Default::default()
    };

    if parsed_entities.is_empty() {
        eprintln!("❌ No parsable entities found in any input file!");
        return Ok(());
    }

    println!("\n📖 Reading database from: {}", database_path);

    let mut translated_count = 0usize;
    let mut rules_changed = 0usize;
    let mut removed_by_date_amount = 0usize;

    let summary = utils::run_parser_pipeline(
        database_path,
        output_path,
        parsed_entities,
        utils::PipelineOptions {
            include_system_accounts: true,
            sort_transactions_by_date: true,
        },
        Some(|db: &mut Value| {
            translated_count = utils::enrich_descriptions_to_english(db)?;
            rules_changed = utils::apply_rules_from_database_path(db, database_path)?;
            removed_by_date_amount = utils::dedup_transactions_by_date_and_amount(db)?;
            Ok(())
        }),
    )?;

    let extra_lines = vec![
        format!("✓ description-en updated: {} transaction(s)", translated_count),
        format!("✓ Rules changed: {} transaction(s)", rules_changed),
        format!(
            "✓ Date+amount dedup removed: {} transaction(s)",
            removed_by_date_amount
        ),
    ];

    utils::print_pipeline_summary(&summary, &extra_lines);
    
    Ok(())
}
