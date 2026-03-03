use anyhow::{Context, Result};
use std::env;

use seb::SebXlsxParser;

struct SebImportContract {
    parser: SebXlsxParser,
    checking_id: String,
    savings_id: String,
}

impl SebImportContract {
    fn new() -> Self {
        let checking_id = "SEB_CHECKING".to_string();
        let savings_id = "SEB_SAVINGS".to_string();

        let parser = SebXlsxParser::new(checking_id.clone(), savings_id.clone());

        Self {
            parser,
            checking_id,
            savings_id,
        }
    }

    fn resolve_account_id<'a>(&'a self, input_file_path: &str) -> &'a str {
        let lower = input_file_path.to_lowercase();

        if lower.contains("saving") || lower.contains("savings") || lower.contains("spark") {
            &self.savings_id
        } else if lower.contains("check") || lower.contains("current") || lower.contains("privat") {
            &self.checking_id
        } else {
            &self.checking_id
        }
    }
}

impl utils::ParserContract for SebImportContract {
    fn parser_name(&self) -> &'static str {
        seb::PARSER_NAME
    }

    fn supported_input_formats(&self) -> &'static [utils::InputFormat] {
        &[utils::InputFormat::Excel]
    }

    fn parse_file(&mut self, input_file_path: &str) -> Result<utils::ParsedEntities> {
        let account_id = self.resolve_account_id(input_file_path);

        let txns = self
            .parser
            .parse_file(input_file_path, account_id)
            .with_context(|| format!("Failed parsing {}", input_file_path))?;

        Ok(utils::ParsedEntities {
            transactions: txns,
            ..Default::default()
        })
    }

    fn finalize_entities(
        &mut self,
        mut entities: utils::ParsedEntities,
    ) -> Result<utils::ParsedEntities> {
        entities.accounts = self.parser.create_accounts();
        Ok(entities)
    }

    fn pipeline_profile(&self) -> utils::PipelineProfile {
        utils::PipelineProfile::Default
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut contract = SebImportContract::new();
    utils::run_parser_contract_cli(&mut contract, &args, "../../../../database")
}
