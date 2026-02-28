use anyhow::{Context, Result};
use std::{
    collections::HashSet,
    env,
    fs::File,
    io::Read,
};

use template::TemplateCsvParser;

struct TemplateImportContract {
    parser: TemplateCsvParser,
    used_account_ids: HashSet<String>,
}

impl TemplateImportContract {
    fn new() -> Self {
        Self {
            // TODO: change TEMPLATE to your parser account prefix, e.g. "MYBANK".
            parser: TemplateCsvParser::new("TEMPLATE"),
            used_account_ids: HashSet::new(),
        }
    }
}

impl utils::ParserContract for TemplateImportContract {
    fn parser_name(&self) -> &'static str {
        template::PARSER_NAME
    }

    fn supported_input_formats(&self) -> &'static [utils::InputFormat] {
        &[utils::InputFormat::Csv]
    }

    fn parse_file(&mut self, input_file_path: &str) -> Result<utils::ParsedEntities> {
        let mut csv_file = File::open(input_file_path)
            .with_context(|| format!("Cannot open {}", input_file_path))?;
        let mut csv_buf = Vec::new();
        csv_file.read_to_end(&mut csv_buf)?;

        // NOTE: parse_reader is intentionally an empty scaffold in the template
        // crate. It returns no transactions until TODOs in transactions.rs are
        // implemented in your copied parser crate.
        let (txns, used_accounts) = self.parser.parse_reader(csv_buf.as_slice())?;
        self.used_account_ids.extend(used_accounts);

        Ok(utils::ParsedEntities {
            transactions: txns,
            ..Default::default()
        })
    }

    fn finalize_entities(&mut self, mut entities: utils::ParsedEntities) -> Result<utils::ParsedEntities> {
        let used_account_ids: Vec<String> = self.used_account_ids.iter().cloned().collect();
        entities.accounts = self.parser.create_used_accounts(&used_account_ids);
        Ok(entities)
    }

    fn pipeline_profile(&self) -> utils::PipelineProfile {
        utils::PipelineProfile::RetailBankDefault
    }
}

fn main() -> Result<()> {
    // Usage:
    //   template_parser [database_path] [output_path]
    //
    // Auto-discovers all .csv files in current directory.
    //
    // Defaults:
    //   Auto-discover all .csv files in current directory
    //   database_path: ../../../../database (resolves to database.json)
    //   output = same as database_path

    let args: Vec<String> = env::args().collect();
    let mut contract = TemplateImportContract::new();
    utils::run_parser_contract_cli(&mut contract, &args, "../../../../database")
}
