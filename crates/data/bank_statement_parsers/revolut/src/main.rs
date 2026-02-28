use anyhow::{Context, Result};
use std::{
    collections::HashSet,
    env,
    fs::File,
    io::Read,
};
use utils::ParserContract;

use revolut::RevolutCsvParser;

struct RevolutImportContract {
    parser: RevolutCsvParser,
    used_account_ids: HashSet<String>,
}

impl RevolutImportContract {
    fn new() -> Self {
        Self {
            parser: RevolutCsvParser::new("REVOLUT"),
            used_account_ids: HashSet::new(),
        }
    }
}

impl utils::ParserContract for RevolutImportContract {
    fn parser_name(&self) -> &'static str {
        revolut::PARSER_NAME
    }

    fn supported_input_formats(&self) -> &'static [utils::InputFormat] {
        &[utils::InputFormat::Csv]
    }

    fn parse_file(&mut self, input_file_path: &str) -> Result<utils::ParsedEntities> {
        let mut csv_file = File::open(input_file_path)
            .with_context(|| format!("Cannot open {}", input_file_path))?;
        let mut csv_buf = Vec::new();
        csv_file.read_to_end(&mut csv_buf)?;

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
    //   revolut_parser [database_path] [output_path]
    //
    // Auto-discovers all .csv files in current directory.
    //
    // Defaults:
    //   Auto-discover all .csv files in current directory
    //   database_path: ../../../../database (resolves to database.json)
    //   output = same as database_path

    let args: Vec<String> = env::args().collect();
    let mut contract = RevolutImportContract::new();
    let input_files = utils::discover_input_files_in_current_dir(contract.supported_input_formats())?;

    if input_files.is_empty() {
        eprintln!("❌ No .csv files found!");
        return Ok(());
    }

    println!("📂 Input files:");
    for file in &input_files {
        println!("  ✓ Found: {}", file);
    }

    // Parse arguments (no per-file overrides; files are always discovered from current directory)
    let database_path = args.get(1).map(|s| s.as_str()).unwrap_or("../../../../database");
    let output_path = args.get(2).map(|s| s.as_str());

    // Parse all discovered .csv files
    let mut parsed_entities = utils::ParsedEntities::default();

    utils::for_each_input_file(&input_files, |input_file_path| {
        println!("\n📖 Parsing {} ({})", input_file_path, contract.parser_name());

        match contract.parse_file(input_file_path) {
            Ok(file_entities) => {
                println!(
                    "  ✓ Found {} transactions",
                    file_entities.transactions.len()
                );
                parsed_entities.append(file_entities);
            }
            Err(e) => {
                eprintln!("  ⚠ Warning: Could not parse file: {}", e);
                eprintln!("    Continuing with next file...");
            }
        }

        Ok(())
    })?;

    let parsed_entities = contract.finalize_entities(parsed_entities)?;

    if parsed_entities.is_empty() {
        eprintln!("❌ No parsable entities found in any input file!");
        return Ok(());
    }

    println!("\n📖 Reading database from: {}", database_path);

    let policy = contract.pipeline_profile().policy();
    let (summary, effects) = utils::run_parser_pipeline_with_policy(
        database_path,
        output_path,
        parsed_entities,
        &policy,
    )?;

    let dedup_label = match policy.dedup_strategy {
        utils::DedupStrategy::None => "Dedup removed",
        utils::DedupStrategy::DateAndAmount => "Date+amount dedup removed",
        utils::DedupStrategy::StrictSignature => "Strict-signature dedup removed",
    };

    let extra_lines = vec![
        format!("✓ description-en updated: {} transaction(s)", effects.description_en_updated),
        format!("✓ Rules changed: {} transaction(s)", effects.rules_changed),
        format!("✓ {}: {} transaction(s)", dedup_label, effects.dedup_removed),
    ];

    utils::print_pipeline_summary(&summary, &extra_lines);
    
    Ok(())
}
