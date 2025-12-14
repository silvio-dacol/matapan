use revolut::*;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Usage:
    //   revolut <input_csv> [output_json] [settings_json]
    // If output_json is omitted, defaults to writing to `dashboard/database.json`.
    // If output_json is set to "-", prints to stdout.

    let args: Vec<String> = std::env::args().collect();

    let input_path = if args.len() > 1 {
        &args[1]
    } else {
        "revolut.csv"
    };

    let default_output = "../../../../database/database.json";
    let output_path = if args.len() > 2 {
        &args[2]
    } else {
        default_output
    };

    // Load settings: third arg or default to `settings.json` next to the binary
    let settings_path = if args.len() > 3 {
        &args[3]
    } else {
        "settings.json"
    };
    let settings = load_settings(settings_path);
    let txs = parse_revolut_csv(input_path, &settings)?;
    if output_path == "-" {
        let json = serde_json::to_string_pretty(&txs)?;
        println!("{}", json);
    } else {
        let out_path = std::path::Path::new(output_path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // If output is a dashboard-style file, merge into template-style document
        let merged = if out_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|n| {
                n.eq_ignore_ascii_case("dashboard.json") || n.eq_ignore_ascii_case("database.json")
            })
            .unwrap_or(false)
        {
            merge_into_dashboard(out_path, &txs)?
        } else {
            serde_json::to_string_pretty(&txs)?
        };

        std::fs::write(out_path, merged)?;
        eprintln!("Wrote Revolut transactions to {}", out_path.display());
    }

    Ok(())
}
