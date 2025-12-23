use anyhow::{Context, Result};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Ensures that database.json exists at the specified path.
/// If it doesn't exist or is invalid JSON, initializes it from template.json
/// in the same directory.
///
/// # Arguments
/// * `database_path` - Path to the database.json file (can be a file path or directory)
///
/// # Returns
/// * `Result<PathBuf>` - The resolved path to the database.json file
///
/// # Example
/// ```no_run
/// use utils::ensure_database_exists;
/// 
/// let db_path = ensure_database_exists("../../database").unwrap();
/// println!("Database ready at: {:?}", db_path);
/// ```
pub fn ensure_database_exists<P: AsRef<Path>>(database_path: P) -> Result<PathBuf> {
    let path = database_path.as_ref();
    
    // Resolve to database.json if a directory was provided
    let db_path = if path.is_dir() || (!path.exists() && !path.to_string_lossy().ends_with(".json")) {
        path.join("database.json")
    } else {
        path.to_path_buf()
    };

    // Check if database.json exists and is valid
    let needs_initialization = match File::open(&db_path) {
        Ok(mut file) => {
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            // Try to parse - if it fails, we need to reinitialize
            serde_json::from_str::<serde_json::Value>(&contents).is_err()
        }
        Err(_) => {
            // File doesn't exist, needs initialization
            true
        }
    };

    if needs_initialization {
        initialize_from_template(&db_path)?;
    }

    Ok(db_path)
}

/// Initializes database.json from template.json
fn initialize_from_template(db_path: &Path) -> Result<()> {
    let template_path = db_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine parent directory of {:?}", db_path))?
        .join("template.json");

    // Read template.json to get user_profile and engine_version
    let mut template_file = File::open(&template_path).with_context(|| {
        format!("Cannot open template file at {:?}", template_path)
    })?;
    
    let mut template_contents = String::new();
    template_file.read_to_string(&mut template_contents)?;
    
    // Validate it's valid JSON
    let template_value: serde_json::Value = serde_json::from_str(&template_contents)
        .with_context(|| format!("template.json at {:?} is not valid JSON", template_path))?;

    // Create minimal database structure with empty arrays
    let minimal_db = serde_json::json!({
        "engine_version": template_value.get("engine_version").unwrap_or(&serde_json::json!("0.1")),
        "user_profile": template_value.get("user_profile").unwrap_or(&serde_json::json!({})),
        "hicp_series": [],
        "instruments": [],
        "accounts": [],
        "positions": [],
        "transactions": [],
        "recurring_templates": [],
        "month_end_snapshots": []
    });

    // Create parent directory if it doesn't exist
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Write to database.json
    let mut db_file = File::create(db_path)
        .with_context(|| format!("Cannot create database file at {:?}", db_path))?;
    
    let formatted = serde_json::to_string_pretty(&minimal_db)?;
    db_file.write_all(formatted.as_bytes())?;

    println!("✓ Initialized database.json with empty structure at {:?}", db_path);
    
    Ok(())
}

/// Reads the database.json file and returns it as a serde_json::Value.
/// Ensures the database exists before reading.
pub fn read_database<P: AsRef<Path>>(database_path: P) -> Result<serde_json::Value> {
    let db_path = ensure_database_exists(database_path)?;
    
    let mut file = File::open(&db_path)
        .with_context(|| format!("Cannot open database at {:?}", db_path))?;
    
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    serde_json::from_str(&contents)
        .with_context(|| format!("Database at {:?} is not valid JSON", db_path))
}

/// Writes a serde_json::Value to the database.json file.
pub fn write_database<P: AsRef<Path>>(
    database_path: P,
    value: &serde_json::Value,
) -> Result<PathBuf> {
    let path = database_path.as_ref();
    
    // Resolve to database.json if a directory was provided
    let db_path = if path.is_dir() || (!path.exists() && !path.to_string_lossy().ends_with(".json")) {
        path.join("database.json")
    } else {
        path.to_path_buf()
    };

    // Create parent directory if it doesn't exist
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = File::create(&db_path)
        .with_context(|| format!("Cannot create database file at {:?}", db_path))?;
    
    let formatted = serde_json::to_string_pretty(value)?;
    file.write_all(formatted.as_bytes())?;

    Ok(db_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_resolution() {
        // This would need a proper test setup with temp directories
        // Just showing the structure for now
    }
}
