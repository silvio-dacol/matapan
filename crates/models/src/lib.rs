
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Settings models
#[derive(Debug, Deserialize)]
pub struct SettingsHicp {
	pub base_year: i32,
	pub base_month: u32,
	pub base_value: f64,
}

#[derive(Debug, Deserialize)]
pub struct SettingsCategories {
	pub assets: Vec<String>,
	pub liabilities: Vec<String>,
	pub positive_cash_flows: Vec<String>,
	pub negative_cash_flows: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SettingsFile {
	pub settings_version: u32,
	pub base_currency: String,
	pub hicp: SettingsHicp,
	pub categories: SettingsCategories,
}

// Raw input entries
#[derive(Debug, Deserialize)]
pub struct NetWorthEntryRaw {
	pub name: String,
	#[serde(rename = "type")]
	pub kind: String,
	pub currency: String,
	pub balance: f64,
	#[serde(default)]
	pub comment: String,
}

#[derive(Debug, Deserialize)]
pub struct CashFlowEntryRaw {
	pub name: String,
	#[serde(rename = "type")]
	pub kind: String,
	pub currency: String,
	pub amount: f64,
	#[serde(default)]
	pub comment: String,
}

#[derive(Debug, Deserialize)]
pub struct MonthlyInput {
	#[serde(alias = "month", alias = "reference_month")]
	pub month: String,
	pub fx_rates: HashMap<String, f64>,
	pub hicp: f64,
	#[serde(default, alias = "cash_flow_entries", alias = "cash-flow-entries")]
	pub cash_flow_entries: Vec<CashFlowEntryRaw>,
	#[serde(default)]
	pub net_worth_entries: Vec<NetWorthEntryRaw>,
}

// Output models
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DashboardMetadata {
	pub generated_at: String,
	pub settings_version: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnapshotTotals {
	pub assets: f64,
	pub liabilities: f64,
	pub net_worth: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnapshotCashFlow {
	pub income: f64,
	pub expenses: f64,
	pub net_cash_flow: f64,
	pub save_rate: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnapshotPerformance {
	pub portfolio_nominal_return: f64,
	pub portfolio_real_return: f64,
	pub twr_cumulative: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnapshotRealWealth {
	pub net_worth_real: f64,
	pub change_pct_from_prev: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ByCategory {
	pub assets: HashMap<String, f64>,
	pub liabilities: HashMap<String, f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnapshotOutput {
	pub month: String,
	pub fx_rates: HashMap<String, f64>,
	pub hicp: f64,
	pub totals: SnapshotTotals,
	pub by_category: ByCategory,
	pub cash_flow: SnapshotCashFlow,
	pub performance: SnapshotPerformance,
	pub real_wealth: SnapshotRealWealth,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct YearlyStats {
	pub year: i32,
	pub months_count: usize,
	pub total_income: f64,
	pub total_expenses: f64,
	pub total_savings: f64,
	pub average_save_rate: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DashboardOutput {
	pub metadata: DashboardMetadata,
	pub yearly_stats: Vec<YearlyStats>,
	pub snapshots: Vec<SnapshotOutput>,
}

// Rounding compatibility methods (already finalized in pipeline generation, so identity)
impl SnapshotOutput {
	pub fn rounded(self) -> Self { self }
}
impl DashboardOutput {
	pub fn rounded(self) -> Self { self }
}

