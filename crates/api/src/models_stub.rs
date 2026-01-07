// Placeholder models until the models crate is created
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardOutput {
    pub metadata: Metadata,
    pub snapshots: Vec<SnapshotOutput>,
}

impl DashboardOutput {
    pub fn rounded(self) -> Self {
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotOutput {
    pub month: String,
}

impl SnapshotOutput {
    pub fn rounded(self) -> Self {
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyInput {
    pub month: String,
    pub net_worth_entries: Vec<NetWorthEntry>,
    pub fx_rates: std::collections::HashMap<String, f64>,
    pub hicp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetWorthEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub currency: String,
    pub balance: f64,
    pub comment: String,
}
