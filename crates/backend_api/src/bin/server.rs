use backend_api::{FileDashboardRepository, run_server};
use std::{env, path::PathBuf};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse command-line arguments or environment variables (with sane defaults)
    let dashboard_path_raw = env::var("DASHBOARD_PATH")
        .unwrap_or_else(|_| "dashboard/dashboard.json".to_string());
    let database_dir_raw = env::var("DATABASE_DIR")
        .unwrap_or_else(|_| "database".to_string());
    let host = env::var("HOST")
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap_or(3000);

    // Resolve paths: if absolute keep them, else make them relative to project root.
    // Project root heuristic: current executable CWD or parent traversal until Cargo.toml found.
    // Determine workspace root (Cargo workspace) and crate root.
    let crate_root = env::current_dir().unwrap();
    let workspace_root = find_workspace_root().unwrap_or_else(|| crate_root.clone());

    // Resolve against workspace root first (since dashboard lives at workspace level), then crate root as fallback.
    let dashboard_path = resolve_with_fallback(&dashboard_path_raw, &[&workspace_root, &crate_root]);
    let database_dir = resolve_with_fallback(&database_dir_raw, &[&workspace_root, &crate_root]);

    println!("Net Worth API Server");
    println!("====================");
    println!("Crate root: {}", crate_root.display());
    println!("Workspace root: {}", workspace_root.display());
    println!("Dashboard path (resolved): {}", dashboard_path.display());
    println!("Database dir (resolved): {}", database_dir.display());
    println!("Listening on: {}:{}", host, port);
    println!("Environment overrides: DASHBOARD_PATH='{}' DATABASE_DIR='{}'", dashboard_path_raw, database_dir_raw);
    println!();

    // Pre-flight checks
    if !dashboard_path.exists() {
        eprintln!("[FATAL] dashboard.json not found at: {}", dashboard_path.display());
        eprintln!("        Tried workspace + crate roots. Set DASHBOARD_PATH env var to absolute path, e.g.:\n        DASHBOARD_PATH=C:/Users/Silvio/git-repos/net-worth/dashboard/dashboard.json");
        std::process::exit(1);
    }
    if !database_dir.exists() {
        eprintln!("[WARN] database directory not found at: {}", database_dir.display());
        eprintln!("       Continuing; snapshot entries will 404 until directory/files exist.");
    }

    // Create the repository
    let repo = Arc::new(FileDashboardRepository::new(
        dashboard_path,
        database_dir,
    ));

    // Start the server
    run_server(repo, &host, port).await?;

    Ok(())
}

/// Attempt to find the project root by searching upwards for Cargo.toml
/// Find the Cargo workspace root by traversing up until a Cargo.toml that contains a [workspace] section.
fn find_workspace_root() -> Option<PathBuf> {
    let mut dir = env::current_dir().ok()?;
    for _ in 0..10 { // safety limit
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                if content.contains("[workspace]") {
                    return Some(dir.clone());
                }
            }
        }
        if !dir.pop() { break; }
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
    bases.first().unwrap_or(&&env::current_dir().unwrap()).join(input)
}
