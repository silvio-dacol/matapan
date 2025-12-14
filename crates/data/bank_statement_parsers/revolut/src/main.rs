use anyhow::{Context, Result};
use std::{
    env,
    fs::File,
    io::{Read, Write},
};

use revolut::{merge_transactions_into_template, RevolutCsvParser};

fn main() -> Result<()> {
    // Usage:
    //   revolut_parser <revolut.csv> <database.json> [output.json] [account_id]
    //
    // Defaults:
    //   revolut.csv
    //   database.json
    //   output.json = database.json (in place)
    //   account_id = REVOLUT_CURRENT

    let args: Vec<String> = env::args().collect();

    let input_csv = args.get(1).map(|s| s.as_str()).unwrap_or("../revolut.csv");

    // Arg2 can be either a JSON file path or a folder path.
    // If it's a folder, resolve to folder/database.json.
    let arg2 = args.get(2).map(|s| s.as_str());
    let template_path_buf = match arg2 {
        Some(p) => {
            if p.ends_with(".json") {
                std::path::PathBuf::from(p)
            } else {
                let mut pb = std::path::PathBuf::from(p);
                pb.push("database.json");
                pb
            }
        }
        None => std::path::PathBuf::from("../../../../database/database.json"),
    };
    let template_path = template_path_buf.to_string_lossy().to_string();

    let output_path = args.get(3).map(|s| s.as_str()).unwrap_or(&template_path);
    let account_id = args
        .get(4)
        .cloned()
        .unwrap_or_else(|| "REVOLUT_CURRENT".to_string());

    // Read CSV
    let mut csv_file =
        File::open(input_csv).with_context(|| format!("Cannot open {}", input_csv))?;
    let mut csv_buf = Vec::new();
    csv_file.read_to_end(&mut csv_buf)?;

    // Parse
    let parser = RevolutCsvParser::new(account_id);
    let txns = parser.parse_reader(csv_buf.as_slice())?;

    // Read or initialize database.json
    let template: serde_json::Value = {
        // Try reading existing database.json
        match File::open(template_path) {
            Ok(mut tpl_file) => {
                let mut tpl_str = String::new();
                tpl_file.read_to_string(&mut tpl_str)?;
                match serde_json::from_str(&tpl_str) {
                    Ok(v) => v,
                    Err(_) => {
                        // Fallback: initialize from template.json
                        // Prefer template.json alongside the resolved database.json
                        let default_tpl_path = std::path::Path::new(&template_path)
                            .parent()
                            .map(|p| p.join("template.json"))
                            .unwrap_or_else(|| {
                                std::path::PathBuf::from("../../../../database/template.json")
                            });
                        let mut default_file =
                            File::open(&default_tpl_path).with_context(|| {
                                format!(
                                    "database.json is not valid JSON and cannot open {:?}",
                                    default_tpl_path
                                )
                            })?;
                        let mut default_str = String::new();
                        default_file.read_to_string(&mut default_str)?;
                        serde_json::from_str(&default_str).with_context(|| {
                            format!("template.json at {:?} is not valid JSON", default_tpl_path)
                        })?
                    }
                }
            }
            Err(_) => {
                // Missing database.json: initialize from template.json
                let default_tpl_path = std::path::Path::new(&template_path)
                    .parent()
                    .map(|p| p.join("template.json"))
                    .unwrap_or_else(|| {
                        std::path::PathBuf::from("../../../../database/template.json")
                    });
                let mut default_file = File::open(&default_tpl_path).with_context(|| {
                    format!("Cannot open default template at {:?}", default_tpl_path)
                })?;
                let mut default_str = String::new();
                default_file.read_to_string(&mut default_str)?;
                serde_json::from_str(&default_str).with_context(|| {
                    format!("template.json at {:?} is not valid JSON", default_tpl_path)
                })?
            }
        }
    };

    // Merge
    let merged = merge_transactions_into_template(template, txns)?;
    let out_str = serde_json::to_string_pretty(&merged)?;

    // Write
    let mut out_file =
        File::create(output_path).with_context(|| format!("Cannot write {}", output_path))?;
    out_file.write_all(out_str.as_bytes())?;

    println!("OK: wrote {}", output_path);
    Ok(())
}
