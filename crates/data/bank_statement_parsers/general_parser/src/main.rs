use anyhow::Result;
use general_parser::GeneralImportContract;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut contract = GeneralImportContract::new();
    utils::run_parser_contract_cli(&mut contract, &args, "../../../../database")
}
