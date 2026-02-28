use anyhow::{Context, Result};
use std::{env, fs::File, io::Read};

use alipay::AlipayCsvParser;

struct AlipayImportContract {
    parser: AlipayCsvParser,
}

impl AlipayImportContract {
    fn new() -> Self {
        Self {
            parser: AlipayCsvParser::new("ALIPAY_WALLET"),
        }
    }
}

impl utils::ParserContract for AlipayImportContract {
    fn parser_name(&self) -> &'static str {
        alipay::PARSER_NAME
    }

    fn supported_input_formats(&self) -> &'static [utils::InputFormat] {
        &[utils::InputFormat::Csv]
    }

    fn parse_file(&mut self, input_file_path: &str) -> Result<utils::ParsedEntities> {
        let mut csv_file = File::open(input_file_path)
            .with_context(|| format!("Cannot open {}", input_file_path))?;
        let mut csv_buf = Vec::new();
        csv_file.read_to_end(&mut csv_buf)?;

        let txns = self.parser.parse_reader(csv_buf.as_slice())?;

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
    let mut contract = AlipayImportContract::new();
    utils::run_parser_contract_cli(&mut contract, &args, "../../../../database")
}
