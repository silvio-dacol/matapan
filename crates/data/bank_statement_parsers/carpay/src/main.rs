use anyhow::{Context, Result};
use std::env;

use carpay_parser::CarPayXlsxParser;

struct CarPayImportContract {
    parser: CarPayXlsxParser,
}

impl CarPayImportContract {
    fn new() -> Self {
        Self {
            parser: CarPayXlsxParser::new("CARPAY_CARD").with_currency("SEK"),
        }
    }
}

impl utils::ParserContract for CarPayImportContract {
    fn parser_name(&self) -> &'static str {
        carpay_parser::PARSER_NAME
    }

    fn supported_input_formats(&self) -> &'static [utils::InputFormat] {
        &[utils::InputFormat::Excel]
    }

    fn parse_file(&mut self, input_file_path: &str) -> Result<utils::ParsedEntities> {
        let txns = self
            .parser
            .parse_file(input_file_path)
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
        utils::PipelineProfile::RetailBankDefault
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut contract = CarPayImportContract::new();
    utils::run_parser_contract_cli(&mut contract, &args, "../../../../database")
}
