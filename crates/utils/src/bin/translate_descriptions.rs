use ai_client::{OllamaClient, OllamaClientConfig};
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use utils::{read_database, write_database};

/// Classify transaction descriptions and fill `description-en` using a local Ollama model.
///
/// For each unique `description` among transactions:
/// - If it is already English OR a proper name / code that shouldn't be translated, prints `true` and does nothing.
/// - Otherwise prints `false` and writes an English translation into `description-en`.
#[derive(Debug, Parser)]
#[command(
    name = "translate-descriptions",
    author,
    version,
    about = "Translate transaction descriptions to English (local Ollama only)",
    long_about = None
)]
struct Args {
    /// Path to database directory or database.json file
    #[arg(short = 'd', long = "db", default_value = "./database")]
    db_path: PathBuf,

    /// Write changes back to database.json (otherwise dry-run)
    #[arg(short = 'w', long = "write")]
    write: bool,

    /// Overwrite existing non-empty `description-en`
    #[arg(long = "force")]
    force: bool,

    /// Only process at most N unique descriptions (useful for testing)
    #[arg(long = "limit")]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ClassificationResponse {
    is_english_or_name: bool,
    #[serde(default)]
    translation_en: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let config = OllamaClientConfig::from_env();
    let client = OllamaClient::new(config).context("Failed to initialize local Ollama client")?;

    let mut db = read_database(&args.db_path)?;

    let txns = db
        .get_mut("transactions")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("database.json missing 'transactions' array"))?;

    // Build mapping: description -> transaction indices that need processing.
    let mut by_description: HashMap<String, Vec<usize>> = HashMap::new();

    for (idx, txn) in txns.iter().enumerate() {
        let Some(obj) = txn.as_object() else { continue };

        let Some(desc) = obj.get("description").and_then(|v| v.as_str()) else {
            continue;
        };

        let desc_en = obj
            .get("description-en")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let needs_work = args.force || desc_en.trim().is_empty();
        if !needs_work {
            continue;
        }

        let desc = desc.trim();
        if desc.is_empty() {
            continue;
        }

        by_description
            .entry(desc.to_string())
            .or_default()
            .push(idx);
    }

    let total_unique = by_description.len();
    if total_unique == 0 {
        println!("No descriptions to process (everything already has description-en, or descriptions are empty).");
        return Ok(());
    }

    println!("Found {total_unique} unique descriptions to check.");

    let mut translated_unique = 0usize;
    let mut translated_txns = 0usize;
    let mut skipped_unique = 0usize;

    // Simple in-process cache for model results.
    let mut cache: HashMap<String, ClassificationResponse> = HashMap::new();

    let mut processed = 0usize;
    for (desc, indices) in by_description.iter() {
        if let Some(limit) = args.limit {
            if processed >= limit {
                break;
            }
        }
        processed += 1;

        let result = if let Some(r) = cache.get(desc) {
            r
        } else {
            let r = classify_and_translate(&client, desc)
                .with_context(|| format!("Failed to classify/translate description: '{desc}'"))?;
            cache.insert(desc.clone(), r);
            cache.get(desc).expect("just inserted")
        };

        println!(
            "{processed}/{total_unique}: {} -> {}",
            if result.is_english_or_name {
                "true"
            } else {
                "false"
            },
            desc
        );

        if result.is_english_or_name {
            skipped_unique += 1;
            continue;
        }

        let Some(translated) = result
            .translation_en
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        else {
            // Model said it's not English/name but didn't provide a translation; be safe and skip.
            eprintln!("Warning: model returned is_english_or_name=false but no translation_en for: '{desc}'");
            continue;
        };

        translated_unique += 1;

        for idx in indices {
            let Some(obj) = txns.get_mut(*idx).and_then(|v| v.as_object_mut()) else {
                continue;
            };

            set_description_en_preserving_order(obj, translated.to_string());
            translated_txns += 1;
        }
    }

    println!(
        "Done. Unique checked: {}. Unique skipped(true): {}. Unique translated(false): {}. Transactions updated: {}.",
        processed,
        skipped_unique,
        translated_unique,
        translated_txns
    );

    if args.write {
        let path = write_database(&args.db_path, &db)?;
        println!("✓ Wrote changes to {:?}", path);
    } else {
        println!("Dry-run: no changes written. Use --write to persist.");
    }

    Ok(())
}

fn classify_and_translate(
    client: &OllamaClient,
    description: &str,
) -> Result<ClassificationResponse> {
    let system_prompt = r#"You are a strict JSON-only classifier+translator for bank transaction descriptions.

Task:
- Decide if the input text is already English OR is a proper name / acronym / reference code / merchant name that should NOT be translated.
- If it should not be translated: set is_english_or_name=true and translation_en=null.
- Otherwise: set is_english_or_name=false and provide a concise English translation in translation_en.

Rules:
- Preserve IDs, card masks, dates, amounts, currency symbols, and reference codes exactly.
- Do NOT add commentary.
- Output MUST be valid JSON with exactly these keys: is_english_or_name (boolean), translation_en (string or null).

Examples:
Input: "ICA KVANTUM"
Output: {"is_english_or_name": true, "translation_en": null}

Input: "RIMBORSO"
Output: {"is_english_or_name": false, "translation_en": "Refund"}
"#;

    let raw = client.chat(system_prompt, description)?;

    let parsed: ClassificationResponse = serde_json::from_str(&raw)
        .with_context(|| format!("Model did not return valid JSON. Raw: {raw}"))?;

    Ok(parsed)
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
            new_obj.insert(
                "description-en".to_string(),
                Value::String(translated.clone()),
            );
            inserted = true;
        }
    }

    if !inserted {
        new_obj.insert("description-en".to_string(), Value::String(translated));
    }

    *obj = new_obj;
}
