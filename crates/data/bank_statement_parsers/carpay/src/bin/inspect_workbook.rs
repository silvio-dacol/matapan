use anyhow::{Context, Result};
use calamine::{open_workbook, Data, Reader, Xlsx};
use std::env;

fn main() -> Result<()> {
    let xlsx_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "carpay.xlsx".to_string());

    let mut workbook: Xlsx<_> =
        open_workbook(&xlsx_path).with_context(|| format!("Cannot open {}", xlsx_path))?;

    let sheet_names = workbook.sheet_names().to_vec();
    println!("Sheets ({}):", sheet_names.len());

    for sheet_name in sheet_names {
        println!("\n== {} ==", sheet_name);

        let range = match workbook.worksheet_range(&sheet_name) {
            Ok(r) => r,
            Err(e) => {
                println!("  (cannot read: {e})");
                continue;
            }
        };

        let (h, w) = range.get_size();
        println!("  size: rows={h}, cols={w}");

        // Print first non-empty 15 rows (up to 15), with up to 12 columns.
        let mut printed = 0usize;
        for (r_idx, row) in range.rows().enumerate() {
            if printed >= 15 {
                break;
            }

            let mut cells: Vec<String> = row
                .iter()
                .take(12)
                .map(|c| cell_str(Some(c)).unwrap_or_default())
                .map(|s| s.trim().to_string())
                .collect();

            let any_nonempty = cells.iter().any(|s| !s.is_empty());
            if !any_nonempty {
                continue;
            }

            // Trim trailing empties for readability
            while matches!(cells.last(), Some(s) if s.is_empty()) {
                cells.pop();
            }

            println!("  row {:>4}: {}", r_idx + 1, cells.join(" | "));
            printed += 1;
        }

        // Attempt to guess header row by finding a row that contains both a date-like and amount-like column name.
        if let Some((header_row, headers)) = find_header_row_loose(&range) {
            println!("  guessed header row: {}", header_row + 1);
            println!("  headers: {}", headers.join(", "));
        } else {
            println!("  guessed header row: (none)");
        }
    }

    Ok(())
}

fn cell_str(cell: Option<&Data>) -> Option<String> {
    let c = cell?;
    match c {
        Data::String(s) => Some(s.clone()),
        Data::Float(f) => Some(f.to_string()),
        Data::Int(i) => Some(i.to_string()),
        Data::Bool(b) => Some(b.to_string()),
        Data::Empty => None,
        _ => Some(c.to_string()),
    }
}

fn norm(s: &str) -> String {
    s.trim().to_lowercase().replace(['\u{00a0}'], " ")
}

fn find_header_row_loose(range: &calamine::Range<Data>) -> Option<(usize, Vec<String>)> {
    for (r_idx, row) in range.rows().enumerate() {
        let headers: Vec<String> = row
            .iter()
            .map(|c| cell_str(Some(c)).unwrap_or_default())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if headers.is_empty() {
            continue;
        }

        let joined = norm(&headers.join(" | "));
        let looks_like_txn =
            joined.contains("datum") && (joined.contains("belopp") || joined.contains("summa"));
        if looks_like_txn {
            return Some((r_idx, headers));
        }
    }

    None
}
