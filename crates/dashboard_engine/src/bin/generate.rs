use anyhow::{Context, Result};
use dashboard_engine::{generate_dashboard, write_dashboard_json};
use std::env;
use std::path::PathBuf;

fn main() -> Result<()> {
    let settings = env::args()
        .position(|a| a == "--settings")
        .and_then(|i| env::args().nth(i + 1))
        .unwrap_or("settings.json".to_string());
    let database = env::args()
        .position(|a| a == "--database")
        .and_then(|i| env::args().nth(i + 1))
        .unwrap_or("database".to_string());
    let out = env::args()
        .position(|a| a == "--out")
        .and_then(|i| env::args().nth(i + 1))
        .unwrap_or("dashboard/dashboard.json".to_string());

    let settings_path = PathBuf::from(&settings);
    let database_dir = PathBuf::from(&database);
    let out_path = PathBuf::from(&out);

    println!(
        "Generating dashboard...\n  settings: {}\n  database: {}\n  output  : {}",
        settings_path.display(),
        database_dir.display(),
        out_path.display()
    );

    let dashboard =
        generate_dashboard(&settings_path, &database_dir).context("generate dashboard")?;
    write_dashboard_json(&dashboard, &out_path).context("write dashboard.json")?;

    println!("Done. Generated at {}", dashboard.metadata.generated_at);
    Ok(())
}
