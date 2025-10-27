use std::collections::HashMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Utility function to round a float to 2 decimal places
fn round_to_2_decimals(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Settings {
    pub base_currency: String,
    #[serde(default)]
    pub normalize_to_hicp: Option<String>,
    #[serde(default)]
    pub normalize_to_ecli: Option<String>,
    #[serde(default)]
    pub hicp: Option<HicpBase>,
    #[serde(default)]
    pub ecli: Option<EcliWeight>,
    #[serde(default)]
    pub categories: Option<CategoryConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CategoryConfig {
    pub assets: Vec<String>,
    pub liabilities: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HicpBase {
    #[serde(default)]
    pub base_year: Option<String>,
    #[serde(default)]
    pub base_month: Option<String>,
    pub base_hicp: f64,
}

impl HicpBase {
    pub fn base_hicp(&self) -> f64 {
        self.base_hicp
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InputDocument {
    pub metadata: Metadata,
    #[serde(default)]
    pub net_worth_entries: Vec<InputEntry>,
}

impl InputDocument {
    /// Gets fx_rates from metadata
    pub fn get_fx_rates(&self) -> HashMap<String, f64> {
        self.metadata.fx_rates.clone().unwrap_or_default()
    }

    /// Gets base currency from metadata or settings default
    pub fn get_base_currency(&self, settings: Option<&Settings>) -> String {
        self.metadata
            .base_currency
            .clone()
            .or_else(|| settings.map(|s| s.base_currency.clone()))
            .unwrap_or_else(|| "EUR".to_string())
    }

    /// Checks if inflation adjustment is enabled
    pub fn is_inflation_enabled(&self, settings: Option<&Settings>) -> bool {
        self.metadata
            .adjust_to_inflation
            .as_ref()
            .or_else(|| self.metadata.normalize_to_hicp.as_ref())
            .or_else(|| settings.and_then(|s| s.normalize_to_hicp.as_ref()))
            .map(|s| s.to_lowercase() == "yes")
            .unwrap_or(false)
    }

    /// Checks if ECLI normalization is enabled
    pub fn is_ecli_enabled(&self, settings: Option<&Settings>) -> bool {
        self.metadata
            .normalize_to_new_york_ecli
            .as_ref()
            .or_else(|| self.metadata.normalize_to_ecli.as_ref())
            .or_else(|| settings.and_then(|s| s.normalize_to_ecli.as_ref()))
            .map(|s| s.to_lowercase() == "yes")
            .unwrap_or(false)
    }

    /// Gets HICP base value from metadata or settings
    pub fn get_hicp_base(&self, settings: Option<&Settings>) -> Option<f64> {
        self.metadata
            .hicp
            .or_else(|| settings.and_then(|s| s.hicp.as_ref().map(|h| h.base_hicp())))
    }

    /// Gets ECLI weights from settings, with fallback to defaults
    pub fn get_ecli_weights(&self, settings: Option<&Settings>) -> Option<EcliWeight> {
        settings.and_then(|s| s.ecli.clone()).or_else(|| {
            // Provide default weights for 2024_08 format compatibility
            if self.metadata.ecli.is_some() {
                Some(EcliWeight {
                    rent_index_weight: 0.4,
                    groceries_index_weight: 0.35,
                    cost_of_living_index_weight: 0.25,
                    restaurant_price_index_weight: 0.0,
                    local_purchasing_power_index_weight: 0.0,
                })
            } else {
                None
            }
        })
    }

    /// Gets current HICP from metadata
    pub fn get_current_hicp(&self) -> Option<f64> {
        self.metadata.hicp
    }

    /// Gets ECLI basic data from metadata
    pub fn get_ecli_basic(&self) -> Option<EcliBasic> {
        self.metadata.ecli.clone()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Metadata {
    #[serde(default)]
    pub version: Option<u32>,
    pub date: String, // YYYY-MM-DD
    #[serde(default)]
    pub base_currency: Option<String>,
    #[serde(default)]
    pub adjust_to_inflation: Option<String>,
    #[serde(default)]
    pub normalize_to_new_york_ecli: Option<String>,
    #[serde(default)]
    pub normalize_to_hicp: Option<String>,
    #[serde(default)]
    pub normalize_to_ecli: Option<String>,
    #[serde(default)]
    pub hicp: Option<f64>,
    #[serde(default)]
    pub fx_rates: Option<HashMap<String, f64>>,
    #[serde(default)]
    pub ecli: Option<EcliBasic>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EcliWeight {
    pub rent_index_weight: f64,
    pub groceries_index_weight: f64,
    pub cost_of_living_index_weight: f64,
    #[serde(default)]
    pub restaurant_price_index_weight: f64,
    #[serde(default)]
    pub local_purchasing_power_index_weight: f64,
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
    #[serde(default)]
    pub comment: String,
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
            "liabilities" | "debt" | "credit_card_debt" => Some(Category::Liabilities),
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
