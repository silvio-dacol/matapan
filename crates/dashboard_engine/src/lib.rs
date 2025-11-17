use anyhow::{Context, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

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

#[derive(Debug, Serialize)]
pub struct DashboardMetadata {
    pub generated_at: String,
    pub settings_version: u32,
}

#[derive(Debug, Serialize)]
pub struct SnapshotTotals {
    pub assets: f64,
    pub liabilities: f64,
    pub net_worth: f64,
}

#[derive(Debug, Serialize)]
pub struct SnapshotCashFlow {
    pub income: f64,
    pub expenses: f64,
    pub net_cash_flow: f64,
    pub save_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct SnapshotPerformance {
    pub portfolio_nominal_return: f64,
    pub portfolio_real_return: f64,
    pub twr_cumulative: f64,
}

#[derive(Debug, Serialize)]
pub struct SnapshotRealWealth {
    pub net_worth_real: f64,
    pub change_pct_from_prev: f64,
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
pub struct ByCategory {
    pub assets: HashMap<String, f64>,
    pub liabilities: HashMap<String, f64>,
}

#[derive(Debug, Serialize)]
pub struct YearlyStats {
    pub year: i32,
    pub months_count: usize,
    pub total_income: f64,
    pub total_expenses: f64,
    pub total_savings: f64,
    pub average_save_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct DashboardOutput {
    pub metadata: DashboardMetadata,
    pub yearly_stats: Vec<YearlyStats>,
    pub snapshots: Vec<SnapshotOutput>,
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

impl SnapshotOutput {
    fn finalize(mut self) -> Self {
        // Round nested numeric values to expected precision
        self.totals.assets = round2(self.totals.assets);
        self.totals.liabilities = round2(self.totals.liabilities);
        self.totals.net_worth = round2(self.totals.net_worth);
        // Round each asset entry value to 2 decimals
        for (_, v) in self.by_category.assets.iter_mut() {
            *v = round2(*v);
        }
        // Round each liability entry value to 2 decimals
        for (_, v) in self.by_category.liabilities.iter_mut() {
            *v = round2(*v);
        }
        self.cash_flow.income = round2(self.cash_flow.income);
        self.cash_flow.expenses = round2(self.cash_flow.expenses);
        self.cash_flow.net_cash_flow = round2(self.cash_flow.net_cash_flow);
        self.cash_flow.save_rate = round4(self.cash_flow.save_rate);
        self.performance.portfolio_nominal_return =
            round4(self.performance.portfolio_nominal_return);
        self.performance.portfolio_real_return = round4(self.performance.portfolio_real_return);
        self.performance.twr_cumulative = round4(self.performance.twr_cumulative);
        self.real_wealth.net_worth_real = round2(self.real_wealth.net_worth_real);
        self.real_wealth.change_pct_from_prev = round4(self.real_wealth.change_pct_from_prev);
        self
    }
}

pub fn generate_dashboard(settings_path: &Path, database_dir: &Path) -> Result<DashboardOutput> {
    let settings_raw =
        fs::read_to_string(settings_path).with_context(|| "Reading settings.json")?;
    let settings: SettingsFile =
        serde_json::from_str(&settings_raw).with_context(|| "Parsing settings.json")?;

    let mut months: Vec<MonthlyInput> = vec![];
    // read all files matching YYYY_MM.json
    for entry in fs::read_dir(database_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            if name.ends_with(".json") && name.len() == 12 {
                // e.g. 2025_09.json
                let content = fs::read_to_string(&path)?;
                if let Ok(doc) = serde_json::from_str::<MonthlyInput>(&content) {
                    months.push(doc);
                }
            }
        }
    }
    // sort by month ascending
    months.sort_by_key(|m| m.month.clone());

    let mut snapshots: Vec<SnapshotOutput> = vec![];
    let mut prev_portfolio_value: Option<f64> = None;
    let mut twr_cumulative: f64 = 1.0;
    let mut prev_net_worth_real: Option<f64> = None;
    let base_currency = settings.base_currency.clone();
    let base_hicp = settings.hicp.base_value;

    for m in &months {
        let fx = &m.fx_rates;
        let get_rate = |cur: &str| -> f64 {
            if cur == base_currency {
                1.0
            } else {
                *fx.get(cur).unwrap_or(&1.0)
            }
        };

        // Assets & liabilities classification (dynamic categories)
        let mut assets_map: HashMap<String, f64> = HashMap::new();
        let mut liabilities_map: HashMap<String, f64> = HashMap::new();
        let mut total_assets = 0.0;
        let mut total_liabilities = 0.0;
        for e in &m.net_worth_entries {
            let value_base = e.balance * get_rate(&e.currency);
            let kind = e.kind.to_ascii_lowercase();
            // Determine canonical category name by matching settings lists (case-insensitive)
            if let Some(canon) = settings
                .categories
                .liabilities
                .iter()
                .find(|c| c.eq_ignore_ascii_case(&kind))
            {
                *liabilities_map.entry(canon.to_string()).or_insert(0.0) += value_base;
                total_liabilities += value_base;
            } else if let Some(canon) = settings
                .categories
                .assets
                .iter()
                .find(|c| c.eq_ignore_ascii_case(&kind))
            {
                *assets_map.entry(canon.to_string()).or_insert(0.0) += value_base;
                total_assets += value_base;
            } else {
                // Unknown category: skip (could be logged or accumulated under "other")
            }
        }
        let net_worth = total_assets - total_liabilities;

        // Cash flow aggregation
        let mut income = 0.0;
        let mut expenses = 0.0;
        for cf in &m.cash_flow_entries {
            let amt_base = cf.amount * get_rate(&cf.currency);
            let kind = cf.kind.to_ascii_lowercase();
            if settings
                .categories
                .positive_cash_flows
                .iter()
                .any(|c| c.eq_ignore_ascii_case(&kind))
            {
                income += amt_base;
            } else if settings
                .categories
                .negative_cash_flows
                .iter()
                .any(|c| c.eq_ignore_ascii_case(&kind))
            {
                expenses += amt_base.abs();
            }
        }
        let net_cash_flow = income - expenses;
        let save_rate = if income > 0.0 {
            net_cash_flow / income
        } else {
            0.0
        };

        // Portfolio performance (investments + retirement) based on dynamic keys
        let investments_val = *assets_map.get("investments").unwrap_or(&0.0);
        let retirement_val = *assets_map.get("retirement").unwrap_or(&0.0);
        let portfolio_value = investments_val + retirement_val;
        let nominal_return = if let Some(prev) = prev_portfolio_value {
            if prev > 0.0 {
                (portfolio_value - prev) / prev
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Inflation adjustment for real return
        // Real return approx removing change in inflation factor month-over-month
        let inflation_factor = m.hicp / base_hicp;
        let prev_inflation_factor = if let Some(prev_snap) = snapshots.last() {
            prev_snap.hicp / base_hicp
        } else {
            inflation_factor
        };
        let real_return = if prev_portfolio_value.is_some() {
            ((1.0 + nominal_return) / (inflation_factor / prev_inflation_factor)) - 1.0
        } else {
            0.0
        };

        twr_cumulative *= 1.0 + nominal_return;

        // Real wealth
        let net_worth_real = net_worth / inflation_factor;
        let change_pct_from_prev = if let Some(prev_real) = prev_net_worth_real {
            if prev_real > 0.0 {
                (net_worth_real - prev_real) / prev_real
            } else {
                0.0
            }
        } else {
            0.0
        };

        let snap = SnapshotOutput {
            month: m.month.clone(),
            fx_rates: m.fx_rates.clone(),
            hicp: m.hicp,
            totals: SnapshotTotals {
                assets: total_assets,
                liabilities: total_liabilities,
                net_worth,
            },
            by_category: ByCategory {
                assets: assets_map,
                liabilities: liabilities_map,
            },
            cash_flow: SnapshotCashFlow {
                income,
                expenses,
                net_cash_flow,
                save_rate,
            },
            performance: SnapshotPerformance {
                portfolio_nominal_return: nominal_return,
                portfolio_real_return: real_return,
                twr_cumulative,
            },
            real_wealth: SnapshotRealWealth {
                net_worth_real,
                change_pct_from_prev,
            },
        }
        .finalize();

        prev_portfolio_value = Some(portfolio_value);
        prev_net_worth_real = Some(net_worth_real);
        snapshots.push(snap);
    }

    // Yearly stats
    let mut yearly_map: HashMap<i32, Vec<&SnapshotOutput>> = HashMap::new();
    for s in &snapshots {
        if let Ok(year) = s.month[0..4].parse::<i32>() {
            yearly_map.entry(year).or_default().push(s);
        }
    }
    let mut yearly_stats: Vec<YearlyStats> = yearly_map
        .into_iter()
        .map(|(year, vec_snap)| {
            let months_count = vec_snap.len();
            let total_income: f64 = vec_snap.iter().map(|s| s.cash_flow.income).sum();
            let total_expenses: f64 = vec_snap.iter().map(|s| s.cash_flow.expenses).sum();
            let total_savings = total_income - total_expenses;
            let average_save_rate = if months_count > 0 {
                vec_snap.iter().map(|s| s.cash_flow.save_rate).sum::<f64>() / months_count as f64
            } else {
                0.0
            };
            YearlyStats {
                year,
                months_count,
                total_income: round2(total_income),
                total_expenses: round2(total_expenses),
                total_savings: round2(total_savings),
                average_save_rate: round4(average_save_rate),
            }
        })
        .collect();
    yearly_stats.sort_by_key(|y| y.year);

    let metadata = DashboardMetadata {
        generated_at: Local::now().to_rfc3339(),
        settings_version: settings.settings_version,
    };
    Ok(DashboardOutput {
        metadata,
        yearly_stats,
        snapshots,
    })
}

pub fn write_dashboard_json(output: &DashboardOutput, out_path: &Path) -> Result<()> {
    if let Some(parent) = out_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let json = serde_json::to_string_pretty(output)?;
    fs::write(out_path, json)?;
    Ok(())
}
