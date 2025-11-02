use backend_api::{FileDashboardRepository, run_server};
use std::env;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse command-line arguments or use defaults
    let dashboard_path = env::var("DASHBOARD_PATH")
        .unwrap_or_else(|_| "dashboard.json".to_string());
    let database_dir = env::var("DATABASE_DIR")
        .unwrap_or_else(|_| "database".to_string());
    let host = env::var("HOST")
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap_or(3000);

    println!("Net Worth API Server");
    println!("====================");
    println!("Dashboard file: {}", dashboard_path);
    println!("Database directory: {}", database_dir);
    println!("Listening on: {}:{}", host, port);
    println!();

    // Create the repository
    let repo = Arc::new(FileDashboardRepository::new(
        dashboard_path,
        database_dir,
    ));

    // Start the server
    run_server(repo, &host, port).await?;

    Ok(())
}
