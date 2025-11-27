use anyhow::{Context, Result};
use chrono::Local;
use models::*;
use serde_json;
use std::path::PathBuf;
use std::{collections::HashMap, fs, path::Path};

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

fn finalize_snapshot(mut s: SnapshotOutput) -> SnapshotOutput {
    s.totals.assets = round2(s.totals.assets);
    s.totals.liabilities = round2(s.totals.liabilities);
    s.totals.net_worth = round2(s.totals.net_worth);
    for (_, v) in s.by_category.assets.iter_mut() {
        *v = round2(*v);
    }
    for (_, v) in s.by_category.liabilities.iter_mut() {
        *v = round2(*v);
    }
    s.cash_flow.income = round2(s.cash_flow.income);
    s.cash_flow.expenses = round2(s.cash_flow.expenses);
    s.cash_flow.net_cash_flow = round2(s.cash_flow.net_cash_flow);
    s.cash_flow.save_rate = round4(s.cash_flow.save_rate);
    s.performance.portfolio_nominal_return = round4(s.performance.portfolio_nominal_return);
    s.performance.portfolio_real_return = round4(s.performance.portfolio_real_return);
    s.performance.twr_cumulative = round4(s.performance.twr_cumulative);
    s.real_wealth.net_worth_real = round2(s.real_wealth.net_worth_real);
    s.real_wealth.change_pct_from_prev = round4(s.real_wealth.change_pct_from_prev);
    s
}

pub fn generate_dashboard(settings_path: &Path, database_dir: &Path) -> Result<DashboardOutput> {
    let settings_raw =
        fs::read_to_string(settings_path).with_context(|| "Reading settings.json")?;
    let settings: SettingsFile =
        serde_json::from_str(&settings_raw).with_context(|| "Parsing settings.json")?;

    let mut months: Vec<MonthlyInput> = vec![];
    for entry in fs::read_dir(database_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            if name.ends_with(".json") && name.len() == 12 {
                let content = fs::read_to_string(&path)?;
                if let Ok(doc) = serde_json::from_str::<MonthlyInput>(&content) {
                    months.push(doc);
                }
            }
        }
    }
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

        let mut assets_map: HashMap<String, f64> = HashMap::new();
        let mut liabilities_map: HashMap<String, f64> = HashMap::new();
        let mut total_assets = 0.0;
        let mut total_liabilities = 0.0;
        for e in &m.net_worth_entries {
            let value_base = e.balance * get_rate(&e.currency);
            let kind = e.kind.to_ascii_lowercase();
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
            }
        }
        let net_worth = total_assets - total_liabilities;

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

        let investments_val = *assets_map.get("investments").unwrap_or(&0.0);
        let retirement_val = *assets_map.get("retirement").unwrap_or(&0.0);
        let portfolio_value = investments_val + retirement_val;

        // Calculate net contributions from the investment_contributions field
        let mut total_contributions = 0.0;
        for contrib in &m.investment_contributions {
            let contrib_base = contrib.amount * get_rate(&contrib.currency);
            let kind = contrib.kind.to_ascii_lowercase();
            // Only count contributions to investment or retirement/pension accounts
            if kind.contains("investment")
                || kind.contains("retirement")
                || kind.contains("pension")
            {
                total_contributions += contrib_base;
            }
        }

        // Calculate true investment return by removing contributions
        let nominal_return = if let Some(prev_portfolio) = prev_portfolio_value {
            if prev_portfolio > 0.0 {
                // Change in portfolio value
                let portfolio_change = portfolio_value - prev_portfolio;

                // True return = (portfolio change - contributions) / previous value
                (portfolio_change - total_contributions) / prev_portfolio
            } else {
                0.0
            }
        } else {
            0.0
        };

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
        };

        prev_portfolio_value = Some(portfolio_value);
        prev_net_worth_real = Some(net_worth_real);
        snapshots.push(finalize_snapshot(snap));
    }

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

// CLI compatibility config
pub struct Config {
    pub input_dir: PathBuf,
    pub output_file: PathBuf,
    pub settings_file: Option<PathBuf>,
    pub latest_only: bool,
    pub pretty: bool,
}

pub fn run(cfg: Config) -> Result<()> {
    let settings_path = cfg
        .settings_file
        .unwrap_or_else(|| PathBuf::from("settings.json"));
    let dashboard = generate_dashboard(&settings_path, &cfg.input_dir)?;
    if let Some(parent) = cfg.output_file.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let json = if cfg.pretty {
        serde_json::to_string_pretty(&dashboard)?
    } else {
        serde_json::to_string(&dashboard)?
    };
    fs::write(&cfg.output_file, json)?;
    Ok(())
}
