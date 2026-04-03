use anyhow::Result;
use std::env;

use intesa_sanpaolo::IntesaSanpaoloParser;

struct IntesaImportContract {
    parser: IntesaSanpaoloParser,
}

impl IntesaImportContract {
    fn new() -> Self {
        Self {
            parser: IntesaSanpaoloParser::new(),
        }
    }
}

impl utils::ParserContract for IntesaImportContract {
    fn parser_name(&self) -> &'static str {
        intesa_sanpaolo::PARSER_NAME
    }

    fn supported_input_formats(&self) -> &'static [utils::InputFormat] {
        &[utils::InputFormat::Excel]
    }

    fn parse_file(&mut self, input_file_path: &str) -> Result<utils::ParsedEntities> {
        let parsed = self.parser.parse_file(input_file_path)?;

        println!(
            "  ✓ Found {} txns, {} instruments, {} positions",
            parsed.transactions.len(),
            parsed.instruments.len(),
            parsed.positions.len(),
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
    let mut contract = IntesaImportContract::new();
    utils::run_parser_contract_cli(&mut contract, &args, "../../../../database")
}
