use ai_client::{OllamaClient, OllamaClientConfig};
use anyhow::{anyhow, Result};
use serde_json::{Map, Value};
use std::collections::HashMap;

pub fn enrich_descriptions_to_english(database: &mut Value) -> Result<usize> {
    let txns = database
        .get_mut("transactions")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("database.json missing 'transactions' array"))?;

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

pub fn contains_non_latin_script(text: &str) -> bool {
    text.chars().any(is_non_latin_script_char)
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

    if (0x3040..=0x309F).contains(&u)
        || (0x30A0..=0x30FF).contains(&u)
        || (0x31F0..=0x31FF).contains(&u)
    {
        return true;
    }

    if (0xAC00..=0xD7AF).contains(&u)
        || (0x1100..=0x11FF).contains(&u)
        || (0x3130..=0x318F).contains(&u)
    {
        return true;
    }

    if (0x0400..=0x052F).contains(&u)
        || (0x2DE0..=0x2DFF).contains(&u)
        || (0xA640..=0xA69F).contains(&u)
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