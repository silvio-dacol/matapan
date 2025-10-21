use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

mod models;
mod pipeline;

#[derive(Parser, Debug)]
#[command(name = "net-worth")]
#[command(about = "Aggregate net-worth snapshots into a dashboard JSON", long_about = None)]
struct Cli {
    /// Input folder containing JSON snapshots
    #[arg(short, long, default_value = "input")]
    input: PathBuf,

    /// Output file for aggregated dashboard JSON
    #[arg(short, long, default_value = "output/dashboard.json")]
    output: PathBuf,

    /// Only process the latest dated file
    #[arg(long, default_value_t = false)]
    latest_only: bool,

    /// Pretty print JSON output
    #[arg(long, default_value_t = true)]
    pretty: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let cfg = pipeline::Config {
        input_dir: cli.input,
        output_file: cli.output,
        latest_only: cli.latest_only,
        pretty: cli.pretty,
    };

    pipeline::run(cfg)
}
