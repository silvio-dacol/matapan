use anyhow::{Context, Result};
use intesa_sanpaolo::{FileType, IntesaSanpaoloParser};
use std::{env, fs};

fn main() -> Result<()> {
    // Usage:
    //   intesa_sanpaolo [file1.xlsx file2.xlsx ...] [database_path] [output_path]
    //
    // Files can be:
    //   - Transaction files (intesa_sanpaolo.xlsx, movimenti_*.xlsx)
    //   - Portfolio files (patrimonio.xlsx)
    //
    // If no files specified, will auto-discover all intesa_*.xlsx and patrimonio*.xlsx
    //
    // Defaults:
    //   database_path: ../../../../database
    //   output_path: same as database_path

    let args: Vec<String> = env::args().collect();

    // Separate Excel files from other arguments
    let mut excel_files: Vec<String> = Vec::new();
    let mut other_args: Vec<String> = Vec::new();

    for arg in args.iter().skip(1) {
        let lower = arg.to_lowercase();
        if lower.ends_with(".xlsx") || lower.ends_with(".xls") {
            excel_files.push(arg.clone());
        } else {
            other_args.push(arg.clone());
        }
    }

    // If no Excel files specified, auto-discover
    if excel_files.is_empty() {
        println!("📂 No Excel files specified, scanning current directory...");
        for entry in fs::read_dir(".").context("Cannot read current directory")? {
            let path = entry?.path();
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                let name_lower = name.to_lowercase();
                if (name_lower.starts_with("intesa")
                    || name_lower.starts_with("patrimonio")
                    || name_lower.starts_with("movimenti"))
                    && (name_lower.ends_with(".xlsx") || name_lower.ends_with(".xls"))
                {
                    excel_files.push(name.to_string());
                    println!("  ✓ Found: {}", name);
                }
            }
        }
        excel_files.sort();
    }

    if excel_files.is_empty() {
        println!("❌ No Excel input files found.");
        println!("   Expected: intesa_*.xlsx, patrimonio*.xlsx, movimenti*.xlsx");
        println!("   Or provide explicit file paths.");
        return Ok(());
    }

    // Parse remaining arguments
    let database_path = other_args
        .get(0)
        .map(|s| s.as_str())
        .unwrap_or("../../../../database");
    let output_path = other_args.get(1).map(|s| s.as_str());

    let parser = IntesaSanpaoloParser::new();

    println!(
        "📖 Parsing {} file(s) for Intesa Sanpaolo accounts",
        excel_files.len()
    );

    let all_accounts = parser.create_accounts();
    let mut all_txns = Vec::new();
    let mut all_instruments = Vec::new();
    let mut all_positions = Vec::new();
    let mut file_count_transactions = 0;
    let mut file_count_portfolio = 0;

    for file_path in &excel_files {
        println!("  • Processing: {}", file_path);

        let parsed = parser
            .parse_file(file_path)
            .with_context(|| format!("Failed parsing {}", file_path))?;

        match parsed.file_type {
            FileType::Transactions => {
                file_count_transactions += 1;
                println!(
                    "    → Type: Transaction list ({} transactions)",
                    parsed.transactions.len()
                );
                all_txns.extend(parsed.transactions);
            }
            FileType::Portfolio => {
                file_count_portfolio += 1;
                println!(
                    "    → Type: Portfolio ({} positions, {} instruments)",
                    parsed.positions.len(),
                    parsed.instruments.len()
                );
                all_instruments.extend(parsed.instruments);
                all_positions.extend(parsed.positions);
            }
        }
    }

    if all_txns.is_empty() && all_positions.is_empty() {
        println!("❌ No transactions or positions found.");
        return Ok(());
    }

    println!(
        "\n📊 File breakdown: {} transaction files, {} portfolio files",
        file_count_transactions, file_count_portfolio
    );

    println!("📖 Reading database from: {}", database_path);
    let template = utils::read_database(database_path)?;

    let system_accounts = utils::create_system_accounts();
    let (db1, sys_stats) = utils::merge_accounts_with_deduplication(template, system_accounts)?;
    let (db2, acc_stats) = utils::merge_accounts_with_deduplication(db1, all_accounts)?;

    // Merge instruments and positions (if any)
    let (db3, inst_stats) = if !all_instruments.is_empty() {
        intesa_sanpaolo::merge_instruments_with_deduplication(db2, all_instruments)?
    } else {
        (
            db2,
            utils::MergeStats {
                added: 0,
                skipped: 0,
                total: 0,
            },
        )
    };

    let (db4, pos_stats) = if !all_positions.is_empty() {
        intesa_sanpaolo::merge_positions_with_deduplication(db3, all_positions)?
    } else {
        (
            db3,
            utils::MergeStats {
                added: 0,
                skipped: 0,
                total: 0,
            },
        )
    };

    let (merged, txn_stats) = utils::merge_transactions_with_deduplication(db4, all_txns)?;

    let final_output_path = output_path.unwrap_or(database_path);
    let written = utils::write_database(final_output_path, &merged)?;

    println!("\n📊 Summary:");
    println!("─────────────────────────────────────────");
    println!(
        "✓ System accounts: {} added, {} skipped",
        sys_stats.added, sys_stats.skipped
    );
    println!(
        "✓ Intesa accounts: {} added, {} skipped",
        acc_stats.added, acc_stats.skipped
    );
    if inst_stats.total > 0 {
        println!(
            "✓ Instruments: {} added, {} skipped",
            inst_stats.added, inst_stats.skipped
        );
    }
    if pos_stats.total > 0 {
        println!(
            "✓ Positions: {} added, {} skipped",
            pos_stats.added, pos_stats.skipped
        );
    }
    println!(
        "✓ Transactions: {} added, {} skipped (duplicates)",
        txn_stats.added, txn_stats.skipped
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
