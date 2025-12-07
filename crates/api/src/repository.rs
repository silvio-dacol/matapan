use async_trait::async_trait;
use chrono::NaiveDate;
use models::{DashboardOutput, MonthlyInput, SnapshotOutput};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{ApiError, Result};

/// Repository trait for accessing dashboard data
/// This abstraction allows swapping between file-based and database-backed implementations
#[async_trait]
pub trait DashboardRepository: Send + Sync {
    async fn fetch_dashboard(&self) -> Result<DashboardOutput>;
    async fn fetch_latest_snapshot(&self) -> Result<SnapshotOutput>;
    async fn fetch_snapshot_by_date(&self, date: NaiveDate) -> Result<SnapshotOutput>;
    async fn fetch_monthly_input(&self, date: NaiveDate) -> Result<MonthlyInput>;
    async fn get_generated_at(&self) -> Result<String>;
    async fn invalidate_cache(&self);
}

/// File-based implementation that reads from dashboard.json
pub struct FileDashboardRepository {
    dashboard_path: PathBuf,
    database_dir: PathBuf,
    cache: Arc<RwLock<Option<DashboardOutput>>>,
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
    async fn load_dashboard(&self) -> Result<DashboardOutput> {
        // TODO: For development, always load fresh from file.
        // For production with many users, consider enabling cache.

        // Load from file (always fresh)
        let content = tokio::fs::read_to_string(&self.dashboard_path).await?;
        let dashboard: DashboardOutput = serde_json::from_str(&content)?;

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
    async fn load_monthly_input(&self, date: NaiveDate) -> Result<MonthlyInput> {
        let filename = format!("{}_{:02}.json", date.format("%Y"), date.format("%m"));
        let file_path = self.database_dir.join(&filename);
        if !file_path.exists() { return Err(ApiError::SnapshotNotFound(date.to_string())); }
        let content = tokio::fs::read_to_string(&file_path).await?;
        let doc: MonthlyInput = serde_json::from_str(&content)?;
        Ok(doc)
    }
}

#[async_trait]
impl DashboardRepository for FileDashboardRepository {
    async fn fetch_dashboard(&self) -> Result<DashboardOutput> {
        self.load_dashboard().await
    }

    async fn fetch_latest_snapshot(&self) -> Result<SnapshotOutput> {
        let dashboard = self.load_dashboard().await?;
        dashboard
            .snapshots
            .last()
            .cloned()
            .ok_or(ApiError::DashboardNotFound)
    }

    async fn fetch_snapshot_by_date(&self, date: NaiveDate) -> Result<SnapshotOutput> {
        let dashboard = self.load_dashboard().await?;
        let target = date.format("%Y-%m").to_string();
        dashboard.snapshots.iter().find(|s| s.month == target).cloned().ok_or_else(|| ApiError::SnapshotNotFound(date.to_string()))
    }

    async fn fetch_monthly_input(&self, date: NaiveDate) -> Result<MonthlyInput> {
        self.load_monthly_input(date).await
    }

    async fn get_generated_at(&self) -> Result<String> {
        let dashboard = self.load_dashboard().await?;
        Ok(dashboard.metadata.generated_at)
    }

    async fn invalidate_cache(&self) {
        let mut cache = self.cache.write().await;
        *cache = None;
    }
}
