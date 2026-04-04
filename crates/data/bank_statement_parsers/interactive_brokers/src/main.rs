use anyhow::Result;
use std::env;

use ibkr_parser::IbkrCsvParser;

struct IbkrImportContract {
    parser: IbkrCsvParser,
}

impl IbkrImportContract {
    fn new() -> Self {
        Self {
            parser: IbkrCsvParser::new(),
        }
    }
}

impl utils::ParserContract for IbkrImportContract {
    fn parser_name(&self) -> &'static str {
        ibkr_parser::PARSER_NAME
    }

    fn supported_input_formats(&self) -> &'static [utils::InputFormat] {
        &[utils::InputFormat::Csv]
    }

    fn parse_file(&mut self, input_file_path: &str) -> Result<utils::ParsedEntities> {
        let parsed = self.parser.parse_file(input_file_path)?;

        println!(
            "  ✓ Found {} txns, {} instruments, {} positions (as_of={})",
            parsed.transactions.len(),
            parsed.instruments.len(),
            parsed.positions.len(),
            parsed
                .statement_end
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "unknown".to_string())
        );

        Ok(utils::ParsedEntities {
            accounts: Vec::new(),
            instruments: parsed.instruments,
            positions: parsed.positions,
            transactions: parsed.transactions,
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
    let mut contract = IbkrImportContract::new();
    utils::run_parser_contract_cli(&mut contract, &args, "../../../../database")
}
