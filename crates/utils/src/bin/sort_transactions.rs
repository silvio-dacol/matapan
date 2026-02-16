use anyhow::{anyhow, Result};
use clap::Parser;
use std::path::PathBuf;
use utils::{read_database, sort_transactions_by_date, write_database};

#[derive(Debug, Parser)]
#[command(
    name = "sort-transactions",
    author,
    version,
    about = "Sort all transactions by date",
    long_about = None
)]
struct Args {
    /// Path to database directory or database.json file
    #[arg(short = 'd', long = "db", default_value = "./database")]
    db_path: PathBuf,

    /// Write changes back to database.json (otherwise dry-run)
    #[arg(short = 'w', long = "write")]
    write: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut db = read_database(&args.db_path)?;

    let count = db
        .get("transactions")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len())
        .ok_or_else(|| anyhow!("database.json missing 'transactions' array"))?;

    sort_transactions_by_date(&mut db)?;

    if args.write {
        let path = write_database(&args.db_path, &db)?;
        println!("✓ Sorted {count} transactions by date and wrote {:?}", path);
    } else {
        println!("Dry-run: sorted {count} transactions by date in memory. Use --write to persist.");
    }

    Ok(())
}
