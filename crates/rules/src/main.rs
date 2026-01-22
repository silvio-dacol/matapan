use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use utils::dedup_transactions_by_signature;
use utils::{read_database, write_database};

/// Simple rules runner for transactions in database.json.
/// Defaults to reading ./database/database.json relative to repo root.
#[derive(Debug, Parser)]
#[command(name = "rules", author, version, about = "Apply simple rules to transactions", long_about = None)]
struct Args {
    /// Path to database directory or database.json file
    #[arg(short = 'd', long = "db", default_value = "./database")]
    db_path: PathBuf,

    /// Path to rules.json (optional). If omitted, uses built-in sample rules
    #[arg(short = 'r', long = "rules")]
    rules_path: Option<PathBuf>,

    /// Write changes back to database.json (otherwise dry-run)
    #[arg(short = 'w', long = "write")]
    write: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct RuleSet {
    rules: Vec<Rule>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Rule {
    when: Condition,
    set: Map<String, Value>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Condition {
    field: String,
    #[serde(default)]
    contains: Option<String>,
    #[serde(default)]
    equals: Option<Value>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut db = read_database(&args.db_path)?;
    let rules = load_rules(args.rules_path.as_deref()).unwrap_or_else(|e| {
        eprintln!("No rules.json provided or failed to load: {e}. Using built-in sample rules.");
        builtin_rules()
    });

    let (txn_count, matched, changed) = {
        let txns = db
            .get_mut("transactions")
            .and_then(|v| v.as_array_mut())
            .ok_or_else(|| anyhow::anyhow!("database.json missing 'transactions' array"))?;

        let mut matched = 0usize;
        let mut changed = 0usize;

        for txn in txns.iter_mut() {
            if let Some(obj) = txn.as_object_mut() {
                let before = obj.clone();
                let mut applied_any = false;

                for rule in &rules.rules {
                    if matches(obj, &rule.when) {
                        matched += 1;
                        // Apply set operations
                        for (k, v) in &rule.set {
                            obj.insert(k.clone(), v.clone());
                        }
                        applied_any = true;
                    }
                }

                if applied_any && &before != obj {
                    changed += 1;
                }
            }
        }

        (txns.len(), matched, changed)
    };

    // After rules application, run a conservative deduplication pass.
    let removed = dedup_transactions_by_signature(&mut db)?;
    if removed > 0 {
        println!(
            "Dedup: removed {} duplicate transactions by signature.",
            removed
        );
    }

    println!(
        "Processed {} transactions. Rules matched: {}, changed: {}.",
        txn_count, matched, changed
    );

    if args.write {
        let path = write_database(&args.db_path, &db)?;
        println!("✓ Wrote changes to {:?}", path);
    } else {
        println!("Dry-run: no changes written. Use --write to persist.");
    }

    Ok(())
}

fn matches(obj: &Map<String, Value>, cond: &Condition) -> bool {
    let Some(val) = obj.get(&cond.field) else {
        return false;
    };

    // equals (strict JSON equality)
    if let Some(eq) = &cond.equals {
        if val == eq {
            return true;
        }
    }

    // contains (case-insensitive) for string fields
    if let Some(sub) = cond.contains.as_ref() {
        if let Some(s) = val.as_str() {
            return s.to_ascii_lowercase().contains(&sub.to_ascii_lowercase());
        }
    }

    false
}

fn load_rules(path: Option<&Path>) -> Result<RuleSet> {
    let Some(p) = path else {
        return Err(anyhow::anyhow!("rules path not provided"));
    };

    let mut buf = String::new();
    let mut f = File::open(p).with_context(|| format!("Cannot open rules file at {:?}", p))?;
    f.read_to_string(&mut buf)?;
    let parsed: RuleSet = serde_json::from_str(&buf)?;
    Ok(parsed)
}

fn builtin_rules() -> RuleSet {
    // Minimal sample rules that classify income/expense by amount sign
    // and tag some known descriptions. Adapt freely.
    let mut rules = Vec::new();

    // If description contains "salary" -> set category
    rules.push(Rule {
        when: Condition {
            field: "description".into(),
            contains: Some("salary".into()),
            equals: None,
        },
        set: Map::from_iter([("category".into(), Value::String("Income:Salary".into()))]),
    });

    // If description contains "amazon" -> shopping
    rules.push(Rule {
        when: Condition {
            field: "description".into(),
            contains: Some("amazon".into()),
            equals: None,
        },
        set: Map::from_iter([("category".into(), Value::String("Shopping".into()))]),
    });

    RuleSet { rules }
}
