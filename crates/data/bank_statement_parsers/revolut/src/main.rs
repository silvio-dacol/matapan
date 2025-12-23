use anyhow::{Context, Result};
use std::{
    env,
    fs::File,
    io::Read,
};

use revolut::{merge_transactions_into_template, RevolutCsvParser};

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

    // Read database.json (automatically initializes if missing or invalid)
    let template = database::read_database(database_path)?;

    // Merge
    let merged = merge_transactions_into_template(template, txns)?;

    // Write to output path (defaults to database path)
    let final_output_path = output_path.unwrap_or(database_path);
    let written_path = database::write_database(final_output_path, &merged)?;

    println!("✓ Wrote {} transactions to {}", 
        merged.get("transactions")
            .and_then(|t| t.as_array())
            .map(|a| a.len())
            .unwrap_or(0),
        written_path.display()
    );
    
    Ok(())
}
