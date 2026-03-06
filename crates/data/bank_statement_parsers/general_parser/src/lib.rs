use ai_client::{OllamaClient, OllamaClientConfig};
use anyhow::Result;
use serde_json::Value;
use std::collections::{BTreeSet, HashSet};
use utils::ParserContract;

mod accounts;
mod instruments;
mod positions;
mod transactions;

pub const PARSER_NAME: &str = "general_parser";

pub use transactions::{FormatIssue, IssueLevel, UnfoldedInput, UnfoldedSheet};

pub struct GeneralParser {
    pub account_id: String,
    pub institution: String,
    pub default_currency: String,
}

impl GeneralParser {
    pub fn new() -> Self {
        let account_id = std::env::var("GENERAL_PARSER_ACCOUNT_ID")
            .unwrap_or_else(|_| "GENERIC_BANK_ACCOUNT".to_string());
        let institution = std::env::var("GENERAL_PARSER_INSTITUTION")
            .unwrap_or_else(|_| "Unknown Financial Institution".to_string());
        let default_currency =
            std::env::var("GENERAL_PARSER_CURRENCY").unwrap_or_else(|_| "EUR".to_string());

        Self {
            account_id,
            institution,
            default_currency,
        }
    }

    pub fn create_accounts(&self) -> Vec<Value> {
        accounts::create_all_accounts(self)
    }

    pub fn create_used_accounts(&self, used_account_ids: &[String]) -> Vec<Value> {
        accounts::create_used_accounts(self, used_account_ids)
    }
}

pub struct GeneralImportContract {
    parser: GeneralParser,
    ai: OllamaClient,
    seen_account_ids: HashSet<String>,
    pub format_issues: Vec<FormatIssue>,
}

impl GeneralImportContract {
    pub fn new() -> Self {
        let ai = OllamaClient::new(OllamaClientConfig::from_env()).unwrap_or_else(|e| {
            panic!(
                "Cannot initialize local Ollama client for general_parser: {e}. Check OLLAMA_BASE_URL and OLLAMA_MODEL."
            )
        });

        Self {
            parser: GeneralParser::new(),
            ai,
            seen_account_ids: HashSet::new(),
            format_issues: Vec::new(),
        }
    }

    fn parse_any_file(&mut self, input_file_path: &str) -> Result<utils::ParsedEntities> {
        let parsed = transactions::parse_transactions(
            &self.parser,
            &self.ai,
            input_file_path,
        )?;

        self.format_issues.extend(parsed.issues);

        // Always make the parser primary account discoverable to the merger.
        self.seen_account_ids.insert(self.parser.account_id.clone());
        self.seen_account_ids.extend(parsed.used_account_ids);

        Ok(utils::ParsedEntities {
            transactions: parsed.transactions,
            ..Default::default()
        })
    }

    pub fn format_issue_lines(&self) -> Vec<String> {
        if self.format_issues.is_empty() {
            return vec!["i Format issues: none detected".to_string()];
        }

        let mut grouped = BTreeSet::new();
        let mut error_count = 0usize;
        let mut warning_count = 0usize;

        for issue in &self.format_issues {
            match issue.level {
                IssueLevel::Error => error_count += 1,
                IssueLevel::Warning => warning_count += 1,
                IssueLevel::Info => {}
            }

            let row_part = issue
                .row
                .map(|r| format!(" row={r}"))
                .unwrap_or_else(String::new);
            let file_part = issue
                .file
                .as_ref()
                .map(|f| format!(" file={f}"))
                .unwrap_or_else(String::new);
            let sheet_part = issue
                .sheet
                .as_ref()
                .map(|s| format!(" sheet={s}"))
                .unwrap_or_else(String::new);

            grouped.insert(format!(
                "- [{:?}] {}:{}{}{}{}",
                issue.level, issue.code, issue.message, file_part, sheet_part, row_part
            ));
        }

        let mut lines = vec![format!(
            "i Format issues: {} total ({} errors, {} warnings)",
            self.format_issues.len(),
            error_count,
            warning_count
        )];
        lines.extend(grouped);
        lines
    }
}

impl Default for GeneralImportContract {
    fn default() -> Self {
        Self::new()
    }
}

impl ParserContract for GeneralImportContract {
    fn parser_name(&self) -> &'static str {
        PARSER_NAME
    }

    fn supported_input_formats(&self) -> &'static [utils::InputFormat] {
        &[utils::InputFormat::Csv, utils::InputFormat::Excel]
    }

    fn parse_file(&mut self, input_file_path: &str) -> Result<utils::ParsedEntities> {
        self.parse_any_file(input_file_path)
    }

    fn finalize_entities(
        &mut self,
        mut entities: utils::ParsedEntities,
    ) -> Result<utils::ParsedEntities> {
        let used_account_ids: Vec<String> = self.seen_account_ids.iter().cloned().collect();
        entities.accounts = self.parser.create_used_accounts(&used_account_ids);

        Ok(entities)
    }

    fn pipeline_profile(&self) -> utils::PipelineProfile {
        utils::PipelineProfile::Default
    }
}
