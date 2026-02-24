use anyhow::{Context, Result};
use ai_client::{OllamaClient, OllamaClientConfig};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io::Read,
    path::Path,
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
    let (mut merged, txn_stats) = 
        revolut::merge_transactions_into_template(template_with_accounts, all_txns)?;

    // Fill `description-en` (copy or translate when needed)
    let translated_count = enrich_descriptions_to_english(&mut merged)?;

    // Apply rules.json when present
    let rules_changed = apply_rules_from_database_path(&mut merged, database_path)?;

    // Additional duplicate pass by date+amount as requested
    let removed_by_date_amount = utils::dedup_transactions_by_date_and_amount(&mut merged)?;

    // Ensure transactions are ordered by date in database.json
    utils::sort_transactions_by_date(&mut merged)?;

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
    println!("✓ description-en updated: {} transaction(s)", translated_count);
    println!("✓ Rules changed: {} transaction(s)", rules_changed);
    println!(
        "✓ Date+amount dedup removed: {} transaction(s)",
        removed_by_date_amount
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

#[derive(Debug, Deserialize)]
struct RuleSet {
    rules: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
struct Rule {
    when: Condition,
    set: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
struct Condition {
    field: String,
    #[serde(default)]
    contains: Option<String>,
    #[serde(default)]
    equals: Option<Value>,
}

fn enrich_descriptions_to_english(database: &mut Value) -> Result<usize> {
    let txns = database
        .get_mut("transactions")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow::anyhow!("database.json missing 'transactions' array"))?;

    let maybe_client = OllamaClient::new(OllamaClientConfig::from_env()).ok();
    let mut cache: HashMap<String, String> = HashMap::new();
    let mut updated = 0usize;

    for txn in txns.iter_mut() {
        let Some(obj) = txn.as_object_mut() else {
            continue;
        };

        let description = obj
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        if description.is_empty() {
            continue;
        }

        let current_en = obj
            .get("description-en")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        if !current_en.is_empty() {
            continue;
        }

        let description_en = if let Some(cached) = cache.get(description) {
            cached.clone()
        } else {
            let translated = translate_or_copy_description(description, maybe_client.as_ref());
            cache.insert(description.to_string(), translated.clone());
            translated
        };

        set_description_en_preserving_order(obj, description_en);
        updated += 1;
    }

    Ok(updated)
}

fn translate_or_copy_description(description: &str, client: Option<&OllamaClient>) -> String {
    if !contains_non_latin_script(description) {
        return description.to_string();
    }

    if let Some(c) = client {
        if let Ok(translated) = c.translate_text(description, "English") {
            let cleaned = translated.trim();
            if !cleaned.is_empty() {
                return cleaned.to_string();
            }
        }
    }

    description.to_string()
}

fn contains_non_latin_script(text: &str) -> bool {
    text.chars().any(is_non_latin_script_char)
}

fn is_non_latin_script_char(ch: char) -> bool {
    let u = ch as u32;

    if (0x4E00..=0x9FFF).contains(&u)
        || (0x3400..=0x4DBF).contains(&u)
        || (0x20000..=0x2A6DF).contains(&u)
        || (0x2A700..=0x2B73F).contains(&u)
        || (0x2B740..=0x2B81F).contains(&u)
        || (0x2B820..=0x2CEAF).contains(&u)
        || (0x2CEB0..=0x2EBEF).contains(&u)
    {
        return true;
    }

    if (0x3040..=0x309F).contains(&u) || (0x30A0..=0x30FF).contains(&u) || (0x31F0..=0x31FF).contains(&u)
    {
        return true;
    }

    if (0xAC00..=0xD7AF).contains(&u) || (0x1100..=0x11FF).contains(&u) || (0x3130..=0x318F).contains(&u)
    {
        return true;
    }

    if (0x0400..=0x052F).contains(&u) || (0x2DE0..=0x2DFF).contains(&u) || (0xA640..=0xA69F).contains(&u)
    {
        return true;
    }

    if (0x0370..=0x03FF).contains(&u) || (0x1F00..=0x1FFF).contains(&u) {
        return true;
    }

    false
}

fn set_description_en_preserving_order(obj: &mut Map<String, Value>, translated: String) {
    let mut new_obj = serde_json::Map::with_capacity(obj.len() + 1);
    let mut inserted = false;

    for (k, v) in obj.iter() {
        if k == "description-en" {
            continue;
        }

        new_obj.insert(k.clone(), v.clone());

        if k == "description" {
            new_obj.insert("description-en".to_string(), Value::String(translated.clone()));
            inserted = true;
        }
    }

    if !inserted {
        new_obj.insert("description-en".to_string(), Value::String(translated));
    }

    *obj = new_obj;
}

fn apply_rules_from_database_path(database: &mut Value, database_path: &str) -> Result<usize> {
    let Some(rules) = load_rules_from_database_path(database_path)? else {
        return Ok(0);
    };

    let txns = database
        .get_mut("transactions")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow::anyhow!("database.json missing 'transactions' array"))?;

    let mut changed = 0usize;

    for txn in txns.iter_mut() {
        let Some(obj) = txn.as_object_mut() else {
            continue;
        };

        let before = obj.clone();

        for rule in &rules.rules {
            if matches_condition(obj, &rule.when) {
                for (k, v) in &rule.set {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }

        if &before != obj {
            changed += 1;
        }
    }

    Ok(changed)
}

fn load_rules_from_database_path(database_path: &str) -> Result<Option<RuleSet>> {
    let db_path = Path::new(database_path);
    let rules_path = if db_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
    {
        db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("rules.json")
    } else {
        db_path.join("rules.json")
    };

    if !rules_path.exists() {
        return Ok(None);
    }

    let mut file = File::open(&rules_path)
        .with_context(|| format!("Cannot open rules file at {}", rules_path.display()))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;

    let parsed: RuleSet = serde_json::from_str(&buf)
        .with_context(|| format!("Invalid JSON in {}", rules_path.display()))?;

    Ok(Some(parsed))
}

fn matches_condition(obj: &Map<String, Value>, cond: &Condition) -> bool {
    let Some(val) = obj.get(&cond.field) else {
        return false;
    };

    if let Some(eq) = &cond.equals {
        if val == eq {
            return true;
        }
    }

    if let Some(sub) = cond.contains.as_ref() {
        if let Some(s) = val.as_str() {
            return s.to_ascii_lowercase().contains(&sub.to_ascii_lowercase());
        }
    }

    false
}
