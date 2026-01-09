use api::{run_server, FileRuleRepository};
use std::sync::Arc;
use std::{env, path::PathBuf};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse command-line arguments or environment variables
    let database_path_raw =
        env::var("DATABASE_PATH").unwrap_or_else(|_| "database/database.json".to_string());
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap_or(3000);

    // Resolve paths
    let crate_root = env::current_dir().unwrap();
    let workspace_root = find_workspace_root().unwrap_or_else(|| crate_root.clone());
    let database_path =
        resolve_with_fallback(&database_path_raw, &[&workspace_root, &crate_root]);

    println!("Matapan Rule Engine API Server");
    println!("===============================");
    println!("Workspace root: {}", workspace_root.display());
    println!("Database path: {}", database_path.display());
    println!("Listening on: {}:{}", host, port);
    println!("Environment: DATABASE_PATH='{}'", database_path_raw);
    println!();

    // Pre-flight checks
    if !database_path.exists() {
        eprintln!(
            "[WARN] database.json not found at: {}",
            database_path.display()
        );
        eprintln!("       Rule endpoints will return empty until database.json exists.");
        eprintln!("       Set DATABASE_PATH env var if needed.");
    }

    // Create the repository
    let rule_repo = Arc::new(FileRuleRepository::new(database_path));

    // Start the server
    run_server(rule_repo, &host, port).await?;

    Ok(())
}

/// Attempt to find the project root by searching upwards for Cargo.toml
/// Find the Cargo workspace root by traversing up until a Cargo.toml that contains a [workspace] section.
fn find_workspace_root() -> Option<PathBuf> {
    let mut dir = env::current_dir().ok()?;
    for _ in 0..10 {
        // safety limit
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                if content.contains("[workspace]") {
                    return Some(dir.clone());
                }
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

/// Resolve a raw path string against a list of base directories, returning the first existing match, or the first constructed path.
fn resolve_with_fallback(raw: &str, bases: &[&PathBuf]) -> PathBuf {
    let input = PathBuf::from(raw);
    if input.is_absolute() {
        return input;
    }
    for base in bases {
        let candidate = base.join(&input);
        if candidate.exists() {
            return candidate;
        }
    }
    // If none exist yet (maybe will be created later), just use the first base.
    bases
        .first()
        .unwrap_or(&&env::current_dir().unwrap())
        .join(input)
}
