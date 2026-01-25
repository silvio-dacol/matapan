use anyhow::{Context, Result};
use std::path::PathBuf;
use std::{env, fs};
use std::{fs::File, io::Read, path::Path};

use carpay_parser::CarPayXlsxParser;

fn main() -> Result<()> {
    // Usage:
    //   carpay_parser carpay.xlsx other.xlsx ... [database_path] [output_path]
    //
    // If no .xlsx files are provided, it will try to parse files matching "carpay*.xlsx" in cwd.
    //
    // Defaults:
    //   database_path: ../../../../database
    //   output_path: same as database_path

    let args: Vec<String> = env::args().skip(1).collect();

    let mut xlsx_paths: Vec<String> = Vec::new();

    // Default database path should be usable no matter what the current working directory is.
    // (Users often run `cargo run -p carpay_parser` from the workspace root.)
    let default_database_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../database")
        .to_string_lossy()
        .to_string();

    let mut database_path = default_database_path;
    let mut output_path: Option<String> = None;

    for a in &args {
        if a.to_lowercase().ends_with(".xlsx") {
            xlsx_paths.push(a.clone());
        }
    }

    let non_xlsx: Vec<String> = args
        .into_iter()
        .filter(|a| !a.to_lowercase().ends_with(".xlsx"))
        .collect();

    if let Some(p) = non_xlsx.get(0) {
        database_path = p.clone();
    }
    if let Some(p) = non_xlsx.get(1) {
        output_path = Some(p.clone());
    }

    if xlsx_paths.is_empty() {
        for entry in fs::read_dir(".").context("Cannot read current directory")? {
            let path = entry?.path();
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                let low = name.to_lowercase();
                if low.starts_with("carpay") && low.ends_with(".xlsx") {
                    xlsx_paths.push(name.to_string());
                }
            }
        }
        xlsx_paths.sort();
    }

    if xlsx_paths.is_empty() {
        println!("❌ No .xlsx input files found (expected carpay*.xlsx or explicit paths).");
        return Ok(());
    }

    let account_id = "CARPAY_CARD".to_string();
    let parser = CarPayXlsxParser::new(&account_id).with_currency("SEK");

    println!(
        "📖 Parsing {} file(s) for account {}",
        xlsx_paths.len(),
        account_id
    );

    let accounts = vec![parser.create_account()];
    let mut all_txns = Vec::new();

    for p in &xlsx_paths {
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
    let template = read_database_no_init(&database_path)?;

    let system_accounts = utils::create_system_accounts();
    let (db1, sys_stats) = utils::merge_accounts_with_deduplication(template, system_accounts)?;

    let (db2, acc_stats) = utils::merge_accounts_with_deduplication(db1, accounts)?;

    let (merged, txn_stats) = utils::merge_transactions_with_deduplication(db2, all_txns)?;

    let final_output_path = output_path.as_deref().unwrap_or(&database_path);
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
        "✓ Transactions processed: {} added, {} skipped",
        txn_stats.added, txn_stats.skipped
    );
    println!("✅ Database written to: {}", written.display());

    Ok(())
}

fn resolve_database_json_path(database_path: &str) -> PathBuf {
    let path = PathBuf::from(database_path);

    // Match utils' convention: if a directory is provided (or a non-.json path), resolve to database.json.
    if path.extension().and_then(|e| e.to_str()) != Some("json") {
        path.join("database.json")
    } else {
        path
    }
}

/// Read database.json without any side effects.
///
/// `utils::read_database()` calls `ensure_database_exists()`, which may initialize (write)
/// a new empty database.json if the file is missing or invalid. That is convenient for
/// normal CLI usage but is risky when the user is doing a dry-run with a separate output.
fn read_database_no_init(database_path: &str) -> Result<serde_json::Value> {
    let db_path = resolve_database_json_path(database_path);

    if db_path.exists() {
        let mut file = File::open(&db_path)
            .with_context(|| format!("Cannot open database at {}", db_path.display()))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        return serde_json::from_str(&contents)
            .with_context(|| format!("Database at {} is not valid JSON", db_path.display()));
    }

    // If database.json doesn't exist, build an in-memory empty DB from template.json (no writing).
    let template_path = db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("template.json");

    let mut template_file = File::open(&template_path).with_context(|| {
        format!(
            "Database does not exist at {} and template.json was not found at {}",
            db_path.display(),
            template_path.display()
        )
    })?;

    let mut template_contents = String::new();
    template_file.read_to_string(&mut template_contents)?;
    let template_value: serde_json::Value = serde_json::from_str(&template_contents)
        .with_context(|| format!("template.json at {} is not valid JSON", template_path.display()))?;

    Ok(serde_json::json!({
        "engine_version": template_value.get("engine_version").unwrap_or(&serde_json::json!("0.1")),
        "user_profile": template_value.get("user_profile").unwrap_or(&serde_json::json!({})),
        "hicp_series": [],
        "accounts": [],
        "instruments": [],
        "positions": [],
        "transactions": [],
        "recurring_templates": [],
        "month_end_snapshots": []
    }))
}
