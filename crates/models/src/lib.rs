use std::collections::HashMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Utility function to round a float to 2 decimal places
fn round_to_2_decimals(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InputDocument {
    pub metadata: Metadata,
    pub fx_rates: HashMap<String, f64>,
    pub inflation: Inflation,
    #[serde(default)]
    pub net_worth_entries: Vec<InputEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Metadata {
    pub date: String, // YYYY-MM-DD
    pub base_currency: String,
    #[serde(default)]
    pub adjust_to_inflation: Option<String>,
    #[serde(default)]
    pub normalize_to_new_york_ecli: Option<String>,
    #[serde(default)]
    pub hicp: Option<HicpBase>,
    #[serde(default)]
    pub ecli_weight: Option<EcliWeight>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HicpBase {
    // pub base_year: String,
    // pub base_month: String,
    pub base_hicp: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EcliWeight {
    pub rent_index_weight: f64,
    pub groceries_index_weight: f64,
    pub cost_of_living_index_weight: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Inflation {
    #[serde(default)]
    pub ecli_basic: Option<EcliBasic>,
    #[serde(default)]
    pub current_hicp: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EcliBasic {
    pub rent_index: f64,
    pub groceries_index: f64,
    pub cost_of_living_index: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InputEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub currency: String,
    pub balance: f64,
    // #[serde(default)]
    // pub comment: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Cash,
    Investments,
    Personal,
    Pension,
    Liabilities,
}

// How categories are mapped from input entry types
impl Category {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "liquidity" | "cash" => Some(Category::Cash),
            "investments" => Some(Category::Investments),
            "personal" => Some(Category::Personal),
            "pension" | "retirement" => Some(Category::Pension),
            "liabilities" | "debt" => Some(Category::Liabilities),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapshotBreakdown {
    pub cash: f64,
    pub investments: f64,
    pub personal: f64,
    pub pension: f64,
    pub liabilities: f64,
}

impl SnapshotBreakdown {
    /// Round all financial values to 2 decimal places
    pub fn rounded(&self) -> Self {
        Self {
            cash: round_to_2_decimals(self.cash),
            investments: round_to_2_decimals(self.investments),
            personal: round_to_2_decimals(self.personal),
            pension: round_to_2_decimals(self.pension),
            liabilities: round_to_2_decimals(self.liabilities),
        }
    }
}

impl Default for SnapshotBreakdown {
    fn default() -> Self {
        Self {
            cash: 0.0,
            investments: 0.0,
            personal: 0.0,
            pension: 0.0,
            liabilities: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapshotTotals {
    pub assets: f64,
    pub liabilities: f64,
    pub net_worth: f64,
}

impl SnapshotTotals {
    /// Round all financial values to 2 decimal places
    pub fn rounded(&self) -> Self {
        Self {
            assets: round_to_2_decimals(self.assets),
            liabilities: round_to_2_decimals(self.liabilities),
            net_worth: round_to_2_decimals(self.net_worth),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub date: NaiveDate,
    pub base_currency: String,
    pub breakdown: SnapshotBreakdown,
    pub totals: SnapshotTotals,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inflation_adjusted: Option<SnapshotAdjustment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_york_normalized: Option<SnapshotAdjustment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub real_purchasing_power: Option<SnapshotAdjustment>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl Snapshot {
    /// Round all financial values to 2 decimal places
    pub fn rounded(&self) -> Self {
        Self {
            date: self.date,
            base_currency: self.base_currency.clone(),
            breakdown: self.breakdown.rounded(),
            totals: self.totals.rounded(),
            inflation_adjusted: self.inflation_adjusted.as_ref().map(|adj| adj.rounded()),
            new_york_normalized: self.new_york_normalized.as_ref().map(|adj| adj.rounded()),
            real_purchasing_power: self.real_purchasing_power.as_ref().map(|adj| adj.rounded()),
            warnings: self.warnings.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapshotAdjustment {
    pub breakdown: SnapshotBreakdown,
    pub totals: SnapshotTotals,
    pub scale: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deflator: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ecli_norm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalization_applied: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl SnapshotAdjustment {
    /// Round all financial values to 2 decimal places
    pub fn rounded(&self) -> Self {
        Self {
            breakdown: self.breakdown.rounded(),
            totals: self.totals.rounded(),
            scale: round_to_2_decimals(self.scale),
            deflator: self.deflator.map(round_to_2_decimals),
            ecli_norm: self.ecli_norm.map(round_to_2_decimals),
            normalization_applied: self.normalization_applied,
            notes: self.notes.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Dashboard {
    pub generated_at: String,
    pub base_currency: String,
    pub snapshots: Vec<Snapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<Snapshot>,
}

impl Dashboard {
    /// Round all financial values to 2 decimal places
    pub fn rounded(&self) -> Self {
        Self {
            generated_at: self.generated_at.clone(),
            base_currency: self.base_currency.clone(),
            snapshots: self.snapshots.iter().map(|s| s.rounded()).collect(),
            latest: self.latest.as_ref().map(|s| s.rounded()),
        }
    }
}
