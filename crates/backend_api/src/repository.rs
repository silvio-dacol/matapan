use async_trait::async_trait;
use chrono::NaiveDate;
use models::{Dashboard, InputDocument, Snapshot};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{ApiError, Result};

/// Repository trait for accessing dashboard data
/// This abstraction allows swapping between file-based and database-backed implementations
#[async_trait]
pub trait DashboardRepository: Send + Sync {
    /// Fetch the complete dashboard with all snapshots
    async fn fetch_dashboard(&self) -> Result<Dashboard>;

    /// Fetch the latest snapshot
    async fn fetch_latest_snapshot(&self) -> Result<Snapshot>;

    /// Fetch a specific snapshot by date
    async fn fetch_snapshot_by_date(&self, date: NaiveDate) -> Result<Snapshot>;

    /// Fetch raw entries for a specific date
    async fn fetch_entries_by_date(&self, date: NaiveDate) -> Result<InputDocument>;

    /// Get the generation timestamp of the dashboard
    async fn get_generated_at(&self) -> Result<String>;

    /// Invalidate any cached data (forces reload on next fetch)
    async fn invalidate_cache(&self);
}

/// File-based implementation that reads from dashboard.json
pub struct FileDashboardRepository {
    dashboard_path: PathBuf,
    database_dir: PathBuf,
    cache: Arc<RwLock<Option<Dashboard>>>,
}

impl FileDashboardRepository {
    pub fn new<P: AsRef<Path>, D: AsRef<Path>>(dashboard_path: P, database_dir: D) -> Self {
        Self {
            dashboard_path: dashboard_path.as_ref().to_path_buf(),
            database_dir: database_dir.as_ref().to_path_buf(),
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Load dashboard from file, using cache if available and fresh
    async fn load_dashboard(&self) -> Result<Dashboard> {
        // TODO: For development, always load fresh from file.
        // For production with many users, consider enabling cache.

        // Load from file (always fresh)
        let content = tokio::fs::read_to_string(&self.dashboard_path).await?;
        let dashboard: Dashboard = serde_json::from_str(&content)?;

        Ok(dashboard)

        // ALTERNATIVE: To enable caching, uncomment the code below and comment out the lines above
        /*
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(ref dashboard) = *cache {
                return Ok(dashboard.clone());
            }
        }

        // Load from file
        let content = tokio::fs::read_to_string(&self.dashboard_path).await?;
        let dashboard: Dashboard = serde_json::from_str(&content)?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(dashboard.clone());
        }

        Ok(dashboard)
        */
    }

    /// Load a raw input document from the database directory
    async fn load_input_document(&self, date: NaiveDate) -> Result<InputDocument> {
        let filename = format!("{}_{:02}.json", date.format("%Y"), date.format("%m"));
        let file_path = self.database_dir.join(&filename);

        if !file_path.exists() {
            return Err(ApiError::SnapshotNotFound(date.to_string()));
        }

        let content = tokio::fs::read_to_string(&file_path).await?;
        let document: InputDocument = serde_json::from_str(&content)?;

        Ok(document)
    }
}

#[async_trait]
impl DashboardRepository for FileDashboardRepository {
    async fn fetch_dashboard(&self) -> Result<Dashboard> {
        self.load_dashboard().await
    }

    async fn fetch_latest_snapshot(&self) -> Result<Snapshot> {
        let dashboard = self.load_dashboard().await?;
        dashboard
            .latest
            .or_else(|| dashboard.snapshots.last().cloned())
            .ok_or(ApiError::DashboardNotFound)
    }

    async fn fetch_snapshot_by_date(&self, date: NaiveDate) -> Result<Snapshot> {
        let dashboard = self.load_dashboard().await?;
        dashboard
            .snapshots
            .iter()
            .find(|s| s.date == date)
            .cloned()
            .ok_or_else(|| ApiError::SnapshotNotFound(date.to_string()))
    }

    async fn fetch_entries_by_date(&self, date: NaiveDate) -> Result<InputDocument> {
        self.load_input_document(date).await
    }

    async fn get_generated_at(&self) -> Result<String> {
        let dashboard = self.load_dashboard().await?;
        Ok(dashboard.generated_at)
    }

    async fn invalidate_cache(&self) {
        let mut cache = self.cache.write().await;
        *cache = None;
    }
}
