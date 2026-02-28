use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuleSet {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    pub when: Condition,
    pub set: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Condition {
    All {
        and: Vec<Condition>,
    },
    Any {
        or: Vec<Condition>,
    },
    Predicate {
        field: String,
        #[serde(default)]
        contains: Option<String>,
        #[serde(default)]
        equals: Option<Value>,
    },
}

pub fn apply_rules_from_database_path(database: &mut Value, database_path: &str) -> Result<usize> {
    let Some(rules) = load_rules_from_database_path(database_path)? else {
        return Ok(0);
    };

    apply_rules(database, &rules)
}

pub fn apply_rules(database: &mut Value, rules: &RuleSet) -> Result<usize> {
    let txns = database
        .get_mut("transactions")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("database.json missing 'transactions' array"))?;

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

pub fn load_rules_from_database_path(database_path: &str) -> Result<Option<RuleSet>> {
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
    match cond {
        Condition::All { and } => {
            if and.is_empty() {
                return false;
            }

            and.iter().all(|c| matches_condition(obj, c))
        }
        Condition::Any { or } => {
            if or.is_empty() {
                return false;
            }

            or.iter().any(|c| matches_condition(obj, c))
        }
        Condition::Predicate {
            field,
            contains,
            equals,
        } => {
            let Some(val) = obj.get(field) else {
                return false;
            };

            if let Some(eq) = equals {
                if val == eq {
                    return true;
                }
            }

            if let Some(sub) = contains.as_ref() {
                if let Some(s) = val.as_str() {
                    return s
                        .to_ascii_lowercase()
                        .contains(&sub.to_ascii_lowercase());
                }
            }

            false
        }
    }
}
