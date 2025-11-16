use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use chrono::{NaiveDate, Utc};

use data_normalization::compute_adjustments;
use models::*;
use settings_loader;

pub struct Config {
    pub input_dir: PathBuf,
    pub output_file: PathBuf,
    pub settings_file: Option<PathBuf>,
    pub latest_only: bool,
    pub pretty: bool,
}

/// Main pipeline function that processes net worth documents and generates dashboard output
pub fn run(cfg: Config) -> Result<()> {
    // Load settings if provided
    let settings = settings_loader::load_optional_settings(cfg.settings_file.as_ref())?;

    // Load all JSON documents from the input directory
    let mut docs = load_documents(&cfg.input_dir)?;

    // Sort by date ascending to ensure chronological order
    docs.sort_by_key(|d| {
        parse_date(&d.metadata.date)
            .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
    });

    // Handle the case where only the latest snapshot is requested
    if cfg.latest_only {
        if let Some(last) = docs.pop() {
            let snap = to_snapshot(&last, settings.as_ref())?;
            let base_currency = last.get_base_currency(settings.as_ref());

            let metadata = models::DashboardMetadata {
                generated_at: Utc::now().to_rfc3339(),
                base_currency,
                normalize: settings.as_ref().and_then(|s| s.normalize.clone()),
                base_hicp: settings.as_ref().and_then(|s| s.base_hicp.clone()),
                base_basket_of_goods: settings
                    .as_ref()
                    .and_then(|s| s.base_basket_of_goods.clone()),
                ecli_weights: settings.as_ref().and_then(|s| s.ecli_weights.clone()),
                categories: settings.as_ref().and_then(|s| s.categories.clone()),
            };

            // Compute performance for single snapshot (no previous data)
            let mut single = snap.clone();
            single.performance = Some(models::PerformanceMetrics {
                nominal_return: 0.0,
                hicp_monthly: 0.0,
                real_return: 0.0,
                twr_cumulative: 0.0,
                benchmark: "EU Inflation (HICP)".to_string(),
                notes: Some("Only latest snapshot processed; returns set to 0.0".to_string()),
            });

            let dashboard = Dashboard {
                metadata,
                snapshots: vec![single.clone()],
                yearly_stats: None,
            };

            // Round all values to 2 decimal places before writing
            let rounded_dashboard = dashboard.rounded();

            write_dashboard(&cfg.output_file, &rounded_dashboard, cfg.pretty)?;
            return Ok(());
        } else {
            return Err(anyhow!("No input documents found"));
        }
    }

    // Process all documents into snapshots
    let mut snapshots = Vec::new();
    for doc in docs.iter() {
        let snap = to_snapshot(doc, settings.as_ref())?;
        snapshots.push(snap);
    }

    // Compute performance metrics across snapshots (requires sequential access)
    compute_performance_metrics(&mut snapshots);

    // Determine the base currency from settings or use EUR as default
    let base_currency = settings
        .as_ref()
        .map(|s| s.base_currency.clone())
        .unwrap_or_else(|| "EUR".to_string());

    // Create metadata from settings
    let metadata = models::DashboardMetadata {
        generated_at: Utc::now().to_rfc3339(),
        base_currency,
        normalize: settings.as_ref().and_then(|s| s.normalize.clone()),
        base_hicp: settings.as_ref().and_then(|s| s.base_hicp.clone()),
        base_basket_of_goods: settings
            .as_ref()
            .and_then(|s| s.base_basket_of_goods.clone()),
        ecli_weights: settings.as_ref().and_then(|s| s.ecli_weights.clone()),
        categories: settings.as_ref().and_then(|s| s.categories.clone()),
    };

    // Compute yearly cashflow-based save rates
    let yearly_stats = compute_yearly_save_rates(&docs, settings.as_ref());

    let dashboard = Dashboard {
        metadata,
        snapshots,
        yearly_stats: yearly_stats,
    };

    // Round all values to 2 decimal places before writing
    let rounded_dashboard = dashboard.rounded();

    write_dashboard(&cfg.output_file, &rounded_dashboard, cfg.pretty)?;
    Ok(())
}

/// Writes the dashboard data to a JSON file with optional pretty formatting
fn write_dashboard(path: &PathBuf, dashboard: &Dashboard, pretty: bool) -> Result<()> {
    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Creating output dir: {}", parent.display()))?;
    }

    // Serialize dashboard to JSON (pretty or compact format)
    let json = if pretty {
        serde_json::to_string_pretty(dashboard)?
    } else {
        serde_json::to_string(dashboard)?
    };

    // Write JSON to file
    fs::write(path, json).with_context(|| format!("Writing output file: {}", path.display()))?;
    Ok(())
}

/// Loads and parses all JSON documents from the specified directory
fn load_documents(dir: &PathBuf) -> Result<Vec<InputDocument>> {
    let mut docs = Vec::new();

    // Read all entries in the directory
    let entries =
        fs::read_dir(dir).with_context(|| format!("Reading input dir: {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Skip directories
        if path.is_dir() {
            continue;
        }

        // Only process JSON files
        if !path.extension().is_some_and(|ext| ext == "json") {
            continue;
        }

        // Skip template.json and dashboard.json files, and hidden files
        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
            let name_lower = filename.to_ascii_lowercase();
            if name_lower == "template.json"
                || name_lower == "dashboard.json"
                || filename.starts_with('.')
            {
                continue;
            }
        }

        // Read and parse the JSON file directly without intermediate Value
        let raw =
            fs::read_to_string(&path).with_context(|| format!("Reading {}", path.display()))?;
        let doc: InputDocument = serde_json::from_str(&raw)
            .with_context(|| format!("Parsing JSON in {}", path.display()))?;
        docs.push(doc);
    }
    Ok(docs)
}

/// Parses date strings in multiple formats (YYYY-MM-DD or YYYY/MM/DD)
#[inline]
fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(s, "%Y/%m/%d"))
        .map_err(|e| anyhow!(e))
}

/// Converts an input document into a snapshot with calculated totals and adjustments
fn to_snapshot(doc: &InputDocument, settings: Option<&Settings>) -> Result<Snapshot> {
    let data_updated_at = parse_date(&doc.metadata.date)?;
    let base_currency = doc.get_base_currency(settings);

    let mut breakdown = SnapshotBreakdown::default();
    let mut warnings = Vec::new();

    // Get fx_rates from document (handles both old and new formats)
    let fx_rates = doc.get_fx_rates();

    // Process each net worth entry and categorize by type
    for e in &doc.net_worth_entries {
        // Parse the category type
        let Some(cat) = Category::from_str(&e.kind) else {
            warnings.push(format!(
                "Unknown type '{}' for entry '{}' — skipped",
                e.kind, e.name
            ));
            continue;
        };

        // Convert currency to base currency using exchange rates
        let rate = fx_to_base(&e.currency, &base_currency, &fx_rates);
        if rate.is_none() && e.currency.to_uppercase() != base_currency.to_uppercase() {
            warnings.push(format!(
                "Missing FX rate {}->{} for entry '{}' — assuming 1.0",
                e.currency, base_currency, e.name
            ));
        }
        let fx = rate.unwrap_or(1.0);
        let amount_base = e.balance * fx;

        // Add to the appropriate category breakdown
        match cat {
            Category::Cash => breakdown.cash += amount_base,
            Category::Investments => breakdown.investments += amount_base,
            Category::Personal => breakdown.personal += amount_base,
            Category::Pension => breakdown.pension += amount_base,
            Category::Liabilities => breakdown.liabilities += amount_base,
        }
    }

    // Calculate totals from breakdown
    let assets = breakdown.cash + breakdown.investments + breakdown.personal + breakdown.pension;
    let liabilities = breakdown.liabilities;
    // Compute net cash flow for this snapshot from cash_flow_entries.
    // Classification order: use settings categories if present, else fallback to kind heuristics.
    let mut income_total = 0.0f64;
    let mut expense_total = 0.0f64;
    if !doc.cash_flow_entries.is_empty() {
        // Build classification sets from settings if available
        let (pos_set, neg_set) = settings
            .and_then(|s| s.categories.as_ref())
            .map(|c| {
                let p = c
                    .positive_cash_flows
                    .iter()
                    .map(|v| v.to_ascii_lowercase())
                    .collect::<std::collections::HashSet<_>>();
                let n = c
                    .negative_cash_flows
                    .iter()
                    .map(|v| v.to_ascii_lowercase())
                    .collect::<std::collections::HashSet<_>>();
                (p, n)
            })
            .unwrap_or_default();

        for cf in &doc.cash_flow_entries {
            let rate = fx_to_base(&cf.currency, &base_currency, &fx_rates).unwrap_or(1.0);
            let amount_base = cf.amount * rate;
            let kind_lc = cf.kind.to_ascii_lowercase();
            let is_income = if !pos_set.is_empty() {
                pos_set.contains(&kind_lc)
            } else {
                matches!(kind_lc.as_str(), "salary" | "income" | "bonus" | "pension")
                    || kind_lc.starts_with("income_")
            };
            let is_expense = if !neg_set.is_empty() {
                neg_set.contains(&kind_lc)
            } else {
                matches!(
                    kind_lc.as_str(),
                    "rent"
                        | "expense"
                        | "transportation"
                        | "utilities"
                        | "groceries"
                        | "bill"
                        | "tax"
                ) || kind_lc.starts_with("expense_")
            };
            if is_income {
                income_total += amount_base;
            } else if is_expense {
                expense_total += amount_base;
            }
        }
    }
    let net_cash_flow = income_total - expense_total;
    // Store classified cash flow totals into breakdown before rounding/serialization
    breakdown.positive_cash_flow = income_total;
    breakdown.negative_cash_flow = expense_total;
    let totals = SnapshotTotals {
        assets,
        liabilities,
        net_worth: assets - liabilities,
        net_cash_flow,
    };

    // Calculate various adjustments (inflation, cost-of-living, etc.)
    let (inflation_adjusted, real_purchasing_power) =
        compute_adjustments(doc, settings, &totals, &breakdown, &mut warnings)?;

    // Extract metadata for the period
    let fx_rates_opt = if fx_rates.is_empty() {
        None
    } else {
        Some(fx_rates)
    };
    let hicp_opt = doc.metadata.hicp;
    let ecli_opt = doc.metadata.ecli.clone();
    let reference_month = doc
        .metadata
        .reference_month
        .as_ref()
        .and_then(|rm| format_reference_month(rm));

    // Compute inflated basket of goods if settings and hicp data available
    let basket_of_goods = settings
        .and_then(|s| s.base_basket_of_goods.clone())
        .and_then(|base_map| {
            let base_hicp = doc.get_hicp_base(settings);
            match (base_hicp, hicp_opt) {
                (Some(bh), Some(current)) if bh > 0.0 => {
                    let scale = current / bh;
                    let adjusted: std::collections::HashMap<String, f64> = base_map
                        .into_iter()
                        .map(|(k, v)| {
                            let rounded = (v * scale * 100.0).round() / 100.0;
                            (k, rounded)
                        })
                        .collect();
                    Some(adjusted)
                }
                _ => None,
            }
        });

    Ok(Snapshot {
        data_updated_at,
        reference_month,
        fx_rates: fx_rates_opt,
        hicp: hicp_opt,
        ecli: ecli_opt,
        basket_of_goods,
        breakdown,
        totals,
        inflation_adjusted,
        real_purchasing_power,
        performance: None, // filled later in batch computation
        warnings,
    })
}

/// Converts currency amounts to base currency using exchange rates
fn fx_to_base(
    currency: &str,
    base: &str,
    rates: &std::collections::HashMap<String, f64>,
) -> Option<f64> {
    // Same currency - no conversion needed
    if currency.eq_ignore_ascii_case(base) {
        return Some(1.0);
    }

    // Look up exchange rate (1 unit of 'currency' equals 'rate' units of base currency)
    // Try exact match first, then uppercase
    rates
        .get(currency)
        .or_else(|| rates.get(&currency.to_uppercase()))
        .copied()
}

/// Computes yearly average save rates based on cash-flow entries.
/// Save rate formula per year: total_savings / total_income, where
/// total_savings = sum(income) - sum(expenses) across available months.
/// Months may be fewer than 12; we use only months present.
fn compute_yearly_save_rates(
    docs: &[InputDocument],
    settings: Option<&Settings>,
) -> Option<Vec<models::YearlyStats>> {
    if docs.is_empty() {
        return None;
    }
    use std::collections::HashMap;
    struct Acc {
        months: usize,
        income: f64,
        expenses: f64,
    }
    let mut map: HashMap<i32, Acc> = HashMap::new();
    for doc in docs {
        // Determine year from reference_month if available else from date
        let year_opt = doc
            .metadata
            .reference_month
            .as_ref()
            .and_then(|m| m.split('-').next().and_then(|y| y.parse::<i32>().ok()))
            .or_else(|| {
                doc.metadata
                    .date
                    .split('-')
                    .next()
                    .and_then(|y| y.parse::<i32>().ok())
            });
        let Some(year) = year_opt else { continue }; // skip if cannot parse

        let fx_rates = doc.get_fx_rates();
        let base_currency = doc.get_base_currency(settings);
        let mut income_month = 0.0f64;
        let mut expenses_month = 0.0f64;
        for e in &doc.cash_flow_entries {
            let rate = fx_to_base(&e.currency, &base_currency, &fx_rates).unwrap_or(1.0);
            let amount_base = e.amount * rate;
            match e.kind.to_ascii_lowercase().as_str() {
                // Treat salary-like entries as income
                "salary" => income_month += amount_base,
                // Treat rent & expense as expenses/outflows
                "rent" | "expense" => expenses_month += amount_base,
                // Ignore other kinds for now (can extend later)
                _ => {}
            }
        }
        if income_month == 0.0 && expenses_month == 0.0 {
            continue;
        }
        let entry = map.entry(year).or_insert(Acc {
            months: 0,
            income: 0.0,
            expenses: 0.0,
        });
        entry.months += 1;
        entry.income += income_month;
        entry.expenses += expenses_month;
    }
    if map.is_empty() {
        return None;
    }
    let mut out: Vec<models::YearlyStats> = map
        .into_iter()
        .map(|(year, acc)| {
            let savings = acc.income - acc.expenses;
            let save_rate = if acc.income > 0.0 {
                savings / acc.income
            } else {
                0.0
            };
            models::YearlyStats {
                year,
                months_count: acc.months,
                total_income: acc.income,
                total_expenses: acc.expenses,
                total_savings: savings,
                average_save_rate: save_rate,
            }
        })
        .collect();
    // Sort by year ascending for consistency
    out.sort_by_key(|y| y.year);
    Some(out)
}

/// Convert `YYYY-MM` to `Month YYYY` (e.g. `2024-08` -> `August 2024`). Returns None if pattern invalid.
fn format_reference_month(s: &str) -> Option<String> {
    if s.len() != 7 {
        return None;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        return None;
    }
    let year = parts[0];
    let month_num: u32 = parts[1].parse().ok()?;
    let name = match month_num {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => return None,
    };
    Some(format!("{} {}", name, year))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_fx_to_base_same_currency() {
        let rates = HashMap::new();
        let result = fx_to_base("EUR", "EUR", &rates);
        assert_eq!(result, Some(1.0));

        // Case insensitive
        let result = fx_to_base("eur", "EUR", &rates);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn test_fx_to_base_conversion() {
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), 0.85);
        rates.insert("GBP".to_string(), 1.15);

        let result = fx_to_base("USD", "EUR", &rates);
        assert_eq!(result, Some(0.85));

        let result = fx_to_base("GBP", "EUR", &rates);
        assert_eq!(result, Some(1.15));
    }

    #[test]
    fn test_fx_to_base_missing_rate() {
        let rates = HashMap::new();
        let result = fx_to_base("JPY", "EUR", &rates);
        assert_eq!(result, None);
    }

    #[test]
    fn test_fx_to_base_case_insensitive_lookup() {
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), 0.85);

        let result = fx_to_base("usd", "EUR", &rates);
        assert_eq!(result, Some(0.85));
    }

    #[test]
    fn test_parse_date_dash_format() {
        let result = parse_date("2024-09-10");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_string(), "2024-09-10");
    }

    #[test]
    fn test_parse_date_slash_format() {
        let result = parse_date("2024/09/10");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_string(), "2024-09-10");
    }

    #[test]
    fn test_parse_date_invalid() {
        let result = parse_date("invalid-date");
        assert!(result.is_err());

        let result = parse_date("2024-13-01"); // Invalid month
        assert!(result.is_err());
    }

    #[test]
    fn test_category_from_str() {
        use models::Category;

        assert_eq!(Category::from_str("cash"), Some(Category::Cash));
        assert_eq!(Category::from_str("Cash"), Some(Category::Cash));
        assert_eq!(Category::from_str("liquidity"), Some(Category::Cash));

        assert_eq!(
            Category::from_str("investments"),
            Some(Category::Investments)
        );
        assert_eq!(
            Category::from_str("Investments"),
            Some(Category::Investments)
        );

        assert_eq!(Category::from_str("pension"), Some(Category::Pension));
        assert_eq!(Category::from_str("retirement"), Some(Category::Pension));

        assert_eq!(
            Category::from_str("liabilities"),
            Some(Category::Liabilities)
        );
        assert_eq!(Category::from_str("debt"), Some(Category::Liabilities));

        assert_eq!(Category::from_str("unknown"), None);
    }
}

/// Computes performance metrics (nominal, real returns and cumulative TWR) for a chronological list of snapshots.
fn compute_performance_metrics(snapshots: &mut [Snapshot]) {
    if snapshots.is_empty() {
        return;
    }
    let mut twr: f64 = 1.0; // time-weighted cumulative factor
    for i in 0..snapshots.len() {
        let (nominal_return, hicp_monthly) = if i == 0 {
            (0.0, 0.0)
        } else {
            let prev = &snapshots[i - 1];
            let curr = &snapshots[i];
            let nw_prev = prev.totals.net_worth;
            let nw_curr = curr.totals.net_worth;
            // External inflows currently unknown -> treat as 0.0
            let external_inflows = 0.0;
            let nominal = if nw_prev.abs() < f64::EPSILON {
                0.0
            } else {
                (nw_curr - nw_prev - external_inflows) / nw_prev
            };
            let hicp_prev = prev.hicp;
            let hicp_curr = curr.hicp;
            let hicp_rate = match (hicp_prev, hicp_curr) {
                (Some(p), Some(c)) if p > 0.0 => (c / p) - 1.0,
                _ => 0.0,
            };
            (nominal, hicp_rate)
        };
        let real_return = nominal_return - hicp_monthly;
        twr *= 1.0 + real_return;
        let twr_cumulative = twr - 1.0;
        let notes = if i == 0 {
            Some("First snapshot: returns set to 0.0".to_string())
        } else if nominal_return == 0.0 {
            Some("Net worth unchanged or previous month zero; nominal return 0.0".to_string())
        } else {
            Some("External inflows currently treated as 0.0".to_string())
        };
        snapshots[i].performance = Some(models::PerformanceMetrics {
            nominal_return,
            hicp_monthly,
            real_return,
            twr_cumulative,
            benchmark: "EU Inflation (HICP)".to_string(),
            notes,
        });
    }
}
