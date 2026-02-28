use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::Map;
use serde_json::Value;
use std::{
    env,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    Csv,
    Excel,
}

impl InputFormat {
    fn extensions(self) -> &'static [&'static str] {
        match self {
            InputFormat::Csv => &["csv"],
            InputFormat::Excel => &["xlsx", "xls"],
        }
    }
}

#[derive(Debug, Clone)]
pub struct InputDiscovery {
    pub input_files: Vec<String>,
    pub other_args: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedEntities {
    pub accounts: Vec<Value>,
    pub instruments: Vec<Value>,
    pub positions: Vec<Value>,
    pub transactions: Vec<Value>,
}

impl ParsedEntities {
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
            && self.instruments.is_empty()
            && self.positions.is_empty()
            && self.transactions.is_empty()
    }

    pub fn append(&mut self, other: ParsedEntities) {
        self.accounts.extend(other.accounts);
        self.instruments.extend(other.instruments);
        self.positions.extend(other.positions);
        self.transactions.extend(other.transactions);
    }
}

#[derive(Debug, Clone)]
pub struct PipelineOptions {
    pub include_system_accounts: bool,
    pub sort_transactions_by_date: bool,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            include_system_accounts: true,
            sort_transactions_by_date: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PipelineSummary {
    pub written_path: PathBuf,
    pub system_accounts_added: usize,
    pub system_accounts_skipped: usize,
    pub accounts_added: usize,
    pub accounts_skipped: usize,
    pub instruments_added: usize,
    pub instruments_skipped: usize,
    pub positions_added: usize,
    pub positions_skipped: usize,
    pub transactions_added: usize,
    pub transactions_skipped: usize,
    pub total_accounts: usize,
    pub total_transactions: usize,
}

impl PipelineSummary {
    pub fn total_accounts_added(&self) -> usize {
        self.system_accounts_added + self.accounts_added
    }
}

pub fn discover_input_files(args: &[String], formats: &[InputFormat]) -> Result<InputDiscovery> {
    if formats.is_empty() {
        return Err(anyhow!("discover_input_files requires at least one input format"));
    }

    let mut input_files: Vec<String> = Vec::new();
    let mut other_args: Vec<String> = Vec::new();

    for arg in args.iter().skip(1) {
        if has_supported_extension(arg, formats) {
            input_files.push(arg.clone());
        } else {
            other_args.push(arg.clone());
        }
    }

    if input_files.is_empty() {
        let cwd = env::current_dir().context("Cannot read current directory")?;
        for entry in fs::read_dir(&cwd)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let Some(filename) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };

            if has_supported_extension(filename, formats) {
                input_files.push(filename.to_string());
            }
        }

        input_files.sort();
    }

    Ok(InputDiscovery {
        input_files,
        other_args,
    })
}

pub fn run_parser_pipeline<F>(
    database_path: &str,
    output_path: Option<&str>,
    entities: ParsedEntities,
    options: PipelineOptions,
    mut post_merge_hook: Option<F>,
) -> Result<PipelineSummary>
where
    F: FnMut(&mut Value) -> Result<()>,
{
    let template = crate::read_database(database_path)?;

    let (db_after_sys, sys_added, sys_skipped) = if options.include_system_accounts {
        let system_accounts = crate::create_system_accounts();
        let (db, stats) = crate::merge_accounts_with_deduplication(template, system_accounts)?;
        (db, stats.added, stats.skipped)
    } else {
        (template, 0, 0)
    };

    let (db_after_accounts, acc_stats) =
        crate::merge_accounts_with_deduplication(db_after_sys, entities.accounts)?;

    let (db_after_instruments, inst_stats) =
        crate::merge_instruments_with_deduplication(db_after_accounts, entities.instruments)?;

    let (db_after_positions, pos_stats) =
        crate::merge_positions_with_deduplication(db_after_instruments, entities.positions)?;

    let (mut merged, txn_stats) =
        crate::merge_transactions_with_deduplication(db_after_positions, entities.transactions)?;

    if let Some(hook) = post_merge_hook.as_mut() {
        hook(&mut merged)?;
    }

    if options.sort_transactions_by_date {
        crate::sort_transactions_by_date(&mut merged)?;
    }

    let total_accounts = merged
        .get("accounts")
        .and_then(|a| a.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let total_transactions = merged
        .get("transactions")
        .and_then(|t| t.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let final_output_path = output_path.unwrap_or(database_path);
    let written_path = crate::write_database(final_output_path, &merged)?;

    Ok(PipelineSummary {
        written_path,
        system_accounts_added: sys_added,
        system_accounts_skipped: sys_skipped,
        accounts_added: acc_stats.added,
        accounts_skipped: acc_stats.skipped,
        instruments_added: inst_stats.added,
        instruments_skipped: inst_stats.skipped,
        positions_added: pos_stats.added,
        positions_skipped: pos_stats.skipped,
        transactions_added: txn_stats.added,
        transactions_skipped: txn_stats.skipped,
        total_accounts,
        total_transactions,
    })
}

pub fn print_pipeline_summary(summary: &PipelineSummary, extra_lines: &[String]) {
    println!("\n📊 Summary:");
    println!("─────────────────────────────────────────");
    println!(
        "✓ Processed {} system accounts: {} added, {} skipped (already exist)",
        summary.system_accounts_added + summary.system_accounts_skipped,
        summary.system_accounts_added,
        summary.system_accounts_skipped
    );
    println!(
        "✓ Processed {} accounts: {} added, {} skipped (already exist)",
        summary.accounts_added + summary.accounts_skipped,
        summary.accounts_added,
        summary.accounts_skipped
    );
    println!(
        "✓ Processed {} transactions: {} added, {} skipped (duplicates)",
        summary.transactions_added + summary.transactions_skipped,
        summary.transactions_added,
        summary.transactions_skipped
    );

    for line in extra_lines {
        println!("{line}");
    }

    println!("✓ Total accounts in database: {}", summary.total_accounts);
    println!("✓ Total transactions in database: {}", summary.total_transactions);
    println!("─────────────────────────────────────────");
    println!("✅ Database written to: {}", summary.written_path.display());
}

pub fn apply_rules_from_database_path(database: &mut Value, database_path: &str) -> Result<usize> {
    let Some(rules) = load_rules_from_database_path(database_path)? else {
        return Ok(0);
    };

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

    let parsed: RuleSet =
        serde_json::from_str(&buf).with_context(|| format!("Invalid JSON in {}", rules_path.display()))?;

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

fn has_supported_extension(path_or_name: &str, formats: &[InputFormat]) -> bool {
    let ext = Path::new(path_or_name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());

    let Some(ext) = ext else {
        return false;
    };

    formats
        .iter()
        .flat_map(|f| f.extensions().iter().copied())
        .any(|supported| ext == supported)
}

pub fn discover_input_files_in_current_dir(formats: &[InputFormat]) -> Result<Vec<String>> {
    if formats.is_empty() {
        return Err(anyhow!("discover_input_files_in_current_dir requires at least one input format"));
    }

    let mut input_files: Vec<String> = Vec::new();
    let cwd = env::current_dir().context("Cannot read current directory")?;
    for entry in fs::read_dir(&cwd)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(filename) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };

        if has_supported_extension(filename, formats) {
            input_files.push(filename.to_string());
        }
    }

    input_files.sort();
    Ok(input_files)
}

pub fn for_each_input_file<F>(input_files: &[String], mut handler: F) -> Result<()>
where
    F: FnMut(&str) -> Result<()>,
{
    for input_file in input_files {
        handler(input_file)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_retail_bank_default_is_expected() {
        let policy = PipelineProfile::RetailBankDefault.policy();
        assert!(policy.include_system_accounts);
        assert!(policy.sort_transactions_by_date);
        assert!(policy.apply_rules);
        assert!(policy.enrich_description_en);
        assert_eq!(policy.dedup_strategy, DedupStrategy::DateAndAmount);
    }

    #[test]
    fn profile_broker_default_is_expected() {
        let policy = PipelineProfile::BrokerDefault.policy();
        assert!(policy.include_system_accounts);
        assert!(policy.sort_transactions_by_date);
        assert!(policy.apply_rules);
        assert!(!policy.enrich_description_en);
        assert_eq!(policy.dedup_strategy, DedupStrategy::None);
    }

    #[test]
    fn profile_minimal_import_is_expected() {
        let policy = PipelineProfile::MinimalImport.policy();
        assert!(policy.include_system_accounts);
        assert!(!policy.sort_transactions_by_date);
        assert!(!policy.apply_rules);
        assert!(!policy.enrich_description_en);
        assert_eq!(policy.dedup_strategy, DedupStrategy::None);
    }

    #[test]
    fn parsed_entities_append_merges_all_collections() {
        let mut left = ParsedEntities {
            accounts: vec![serde_json::json!({"account_id": "A"})],
            instruments: vec![serde_json::json!({"instrument_id": "I1"})],
            positions: vec![],
            transactions: vec![serde_json::json!({"txn_id": "T1"})],
        };

        let right = ParsedEntities {
            accounts: vec![serde_json::json!({"account_id": "B"})],
            instruments: vec![],
            positions: vec![serde_json::json!({"position_id": "P1"})],
            transactions: vec![serde_json::json!({"txn_id": "T2"})],
        };

        left.append(right);

        assert_eq!(left.accounts.len(), 2);
        assert_eq!(left.instruments.len(), 1);
        assert_eq!(left.positions.len(), 1);
        assert_eq!(left.transactions.len(), 2);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DedupStrategy {
    None,
    DateAndAmount,
    StrictSignature,
}

#[derive(Debug, Clone)]
pub struct PipelinePolicy {
    pub include_system_accounts: bool,
    pub sort_transactions_by_date: bool,
    pub apply_rules: bool,
    pub enrich_description_en: bool,
    pub dedup_strategy: DedupStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineProfile {
    RetailBankDefault,
    BrokerDefault,
    MinimalImport,
}

impl PipelineProfile {
    pub fn policy(self) -> PipelinePolicy {
        match self {
            PipelineProfile::RetailBankDefault => PipelinePolicy {
                include_system_accounts: true,
                sort_transactions_by_date: true,
                apply_rules: true,
                enrich_description_en: true,
                dedup_strategy: DedupStrategy::DateAndAmount,
            },
            PipelineProfile::BrokerDefault => PipelinePolicy {
                include_system_accounts: true,
                sort_transactions_by_date: true,
                apply_rules: true,
                enrich_description_en: false,
                dedup_strategy: DedupStrategy::None,
            },
            PipelineProfile::MinimalImport => PipelinePolicy {
                include_system_accounts: true,
                sort_transactions_by_date: false,
                apply_rules: false,
                enrich_description_en: false,
                dedup_strategy: DedupStrategy::None,
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PolicyEffects {
    pub description_en_updated: usize,
    pub rules_changed: usize,
    pub dedup_removed: usize,
}

pub fn run_parser_pipeline_with_policy(
    database_path: &str,
    output_path: Option<&str>,
    entities: ParsedEntities,
    policy: &PipelinePolicy,
) -> Result<(PipelineSummary, PolicyEffects)> {
    let mut effects = PolicyEffects::default();

    let summary = run_parser_pipeline(
        database_path,
        output_path,
        entities,
        PipelineOptions {
            include_system_accounts: policy.include_system_accounts,
            sort_transactions_by_date: policy.sort_transactions_by_date,
        },
        Some(|db: &mut Value| {
            if policy.enrich_description_en {
                effects.description_en_updated = crate::enrich_descriptions_to_english(db)?;
            }

            if policy.apply_rules {
                effects.rules_changed = crate::apply_rules_from_database_path(db, database_path)?;
            }

            effects.dedup_removed = match policy.dedup_strategy {
                DedupStrategy::None => 0,
                DedupStrategy::DateAndAmount => crate::dedup_transactions_by_date_and_amount(db)?,
                DedupStrategy::StrictSignature => crate::dedup_transactions_by_signature(db)?,
            };

            Ok(())
        }),
    )?;

    Ok((summary, effects))
}