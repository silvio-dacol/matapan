//! Balance reference snapshots for accounts.
//!
//! A **balance reference** records the actual known balance of an account at a
//! specific point in time.  From that single anchor the end-of-month balance
//! for _any_ calendar month can be reconstructed by accumulating (or
//! reversing) the net transaction flow recorded for that account.
//!
//! ## Monthly balance formula
//!
//! Define the _net flow_ for a given account in a given month:
//!
//! ```text
//! net_flow(account, YYYY-MM) =
//!     Σ amount  for transactions where  to_account_id   == account_id
//!   − Σ amount  for transactions where  from_account_id == account_id
//! ```
//!
//! Then, given a reference `{ account_id, date, amount }`:
//!
//! * Let `ref_month = date[..7]`  (e.g. `"2024-03"`).
//! * Let `cumsum(M) = Σ net_flow(m)` for all months `m ≤ M` that appear in
//!   the transaction list.
//! * `balance_end_of_month(M) = ref_amount + (cumsum(M) − cumsum(ref_month))`
//!
//! This single formula covers all three cases:
//! * `M == ref_month` → `ref_amount`  (round-trip identity ✓)
//! * `M >  ref_month` → `ref_amount + Σ net_flow(ref_month+1 … M)`  (forward)
//! * `M <  ref_month` → `ref_amount − Σ net_flow(M+1 … ref_month)` (backward)
//!
//! ### Currency note
//!
//! The calculation uses the `amount` field of each transaction **as-is**.
//! For a consistent single-currency view, pass transactions from
//! `database_normalized.json` (where every amount has been converted to the
//! user's `base_currency`).  If you pass raw transactions from
//! `database.json`, amounts in multiple currencies will be added together,
//! which is only correct when the account operates in a single currency.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};

use crate::transactions::MergeStats;
use crate::round_digits::round_money;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// All fields required to create a balance reference entry.
#[derive(Debug, Clone)]
pub struct BalanceReferenceInput {
    /// Unique identifier, e.g. `"SEB_SAVINGS-2024-03-31"`.
    pub reference_id: String,
    /// Must match an `account_id` in the `accounts` array.
    pub account_id: String,
    /// ISO-8601 date string, e.g. `"2024-03-31"`.
    pub date: String,
    /// Actual balance on `date` in `currency`.
    pub amount: f64,
    pub currency: String,
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builds a JSON balance-reference object ready for insertion into
/// `database.json["balance_references"]`.
pub fn build_balance_reference(input: &BalanceReferenceInput) -> Value {
    json!({
        "reference_id": input.reference_id,
        "account_id": input.account_id,
        "date": input.date,
        "amount": round_money(input.amount),
        "currency": input.currency,
    })
}

// ---------------------------------------------------------------------------
// Merge / deduplication
// ---------------------------------------------------------------------------

/// Merges new balance references into an existing database, deduplicating
/// by `reference_id`.
///
/// # Arguments
/// * `database` – Full database JSON (must contain a `"balance_references"` array).
/// * `new_refs`  – New reference objects produced by [`build_balance_reference`].
///
/// # Returns
/// The updated database and merge statistics.
pub fn merge_balance_references_with_deduplication(
    mut database: Value,
    new_refs: Vec<Value>,
) -> Result<(Value, MergeStats)> {
    let arr = database
        .get_mut("balance_references")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("database.json missing 'balance_references' array"))?;

    let existing_ids: HashSet<String> = arr
        .iter()
        .filter_map(|r| {
            r.get("reference_id")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .collect();

    let mut stats = MergeStats {
        added: 0,
        skipped: 0,
        total: new_refs.len(),
    };

    for r in new_refs {
        let id = r
            .get("reference_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Balance reference missing 'reference_id' field"))?;

        if existing_ids.contains(id) {
            stats.skipped += 1;
        } else {
            arr.push(r);
            stats.added += 1;
        }
    }

    Ok((database, stats))
}

// ---------------------------------------------------------------------------
// Monthly balance computation
// ---------------------------------------------------------------------------

/// Computes the reconstructed end-of-month balance for `account_id` anchored
/// at the given balance reference.
///
/// Returns a `BTreeMap<String, f64>` keyed by `"YYYY-MM"`, covering every
/// month that either holds at least one transaction affecting the account **or**
/// is the reference month itself.
///
/// # Arguments
/// * `reference`    – A balance reference object (as stored in
///   `database["balance_references"]`).
/// * `transactions` – Slice of transaction objects.  Pass the array from
///   `database_normalized.json` to get base-currency results, or the raw
///   `database.json` array for single-currency accounts.
///
/// # Errors
/// Returns an error if `reference` is missing `account_id`, `date`, or
/// `amount`, or if the date is shorter than 7 characters.
pub fn compute_monthly_balances(
    reference: &Value,
    transactions: &[Value],
) -> Result<BTreeMap<String, f64>> {
    let account_id = reference
        .get("account_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Balance reference missing 'account_id'"))?;

    let ref_date = reference
        .get("date")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Balance reference missing 'date'"))?;

    if ref_date.len() < 7 {
        return Err(anyhow!(
            "Balance reference 'date' must be at least YYYY-MM, got '{}'",
            ref_date
        ));
    }
    let ref_month = &ref_date[..7];

    let ref_amount = reference
        .get("amount")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| anyhow!("Balance reference missing 'amount'"))?;

    // ------------------------------------------------------------------
    // Step 1 – build monthly net flows for this account.
    // ------------------------------------------------------------------
    let mut net_flows: BTreeMap<String, f64> = BTreeMap::new();

    for txn in transactions {
        let date = match txn.get("date").and_then(|v| v.as_str()) {
            Some(d) if d.len() >= 7 => d,
            _ => continue,
        };
        let month = &date[..7];
        let amount = match txn.get("amount").and_then(|v| v.as_f64()) {
            Some(a) => a,
            None => continue,
        };

        let from = txn
            .get("from_account_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let to = txn
            .get("to_account_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if to == account_id {
            *net_flows.entry(month.to_string()).or_insert(0.0) += amount;
        }
        if from == account_id {
            *net_flows.entry(month.to_string()).or_insert(0.0) -= amount;
        }
    }

    // ------------------------------------------------------------------
    // Step 2 – build cumulative net-flow series (BTreeMap keeps months sorted).
    // ------------------------------------------------------------------
    let mut cum_running = 0.0;
    let mut cumulative: BTreeMap<String, f64> = BTreeMap::new();
    for (month, &flow) in &net_flows {
        cum_running += flow;
        cumulative.insert(month.clone(), cum_running);
    }

    // cumsum at ref_month: look up the latest entry at-or-before ref_month.
    let cum_at_ref = cumulative
        .range(..=ref_month.to_string())
        .next_back()
        .map(|(_, &v)| v)
        .unwrap_or(0.0);

    // ------------------------------------------------------------------
    // Step 3 – compute end-of-month balance for each month with data.
    // ------------------------------------------------------------------
    let mut result: BTreeMap<String, f64> = BTreeMap::new();

    // Always include the reference month at the reference amount.
    result.insert(ref_month.to_string(), round_money(ref_amount));

    for month in net_flows.keys() {
        // cumulative always has an entry for every month in net_flows.
        let cum_at_m = cumulative.get(month.as_str()).copied().unwrap_or(0.0);
        let balance = ref_amount + (cum_at_m - cum_at_ref);
        result.insert(month.clone(), round_money(balance));
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_txn(date: &str, from: &str, to: &str, amount: f64) -> Value {
        json!({
            "date": date,
            "from_account_id": from,
            "to_account_id": to,
            "amount": amount,
            "currency": "EUR",
            "txn_id": format!("t-{date}")
        })
    }

    fn make_ref(account_id: &str, date: &str, amount: f64) -> Value {
        json!({
            "reference_id": "test-ref",
            "account_id": account_id,
            "date": date,
            "amount": amount,
            "currency": "EUR"
        })
    }

    #[test]
    fn ref_month_identity() {
        let reference = make_ref("ACC", "2024-03-31", 1000.0);
        let txns = vec![make_txn("2024-03-10", "EXT", "ACC", 200.0)];
        let balances = compute_monthly_balances(&reference, &txns).unwrap();
        // ref_month always returns ref_amount
        assert_eq!(balances["2024-03"], 1000.0);
    }

    #[test]
    fn forward_accumulation() {
        let reference = make_ref("ACC", "2024-03-31", 1000.0);
        let txns = vec![
            make_txn("2024-03-10", "EXT", "ACC", 200.0),
            make_txn("2024-04-05", "EXT", "ACC", 500.0),
            make_txn("2024-04-20", "ACC", "EXT", 100.0),
            make_txn("2024-05-01", "EXT", "ACC", 300.0),
        ];
        let balances = compute_monthly_balances(&reference, &txns).unwrap();
        // Apr net = +500 -100 = +400  → 1000 + 400 = 1400
        assert_eq!(balances["2024-04"], 1400.0);
        // May net = +300            → 1400 + 300 = 1700
        assert_eq!(balances["2024-05"], 1700.0);
    }

    #[test]
    fn backward_reconstruction() {
        let reference = make_ref("ACC", "2024-03-31", 1000.0);
        let txns = vec![
            make_txn("2024-02-10", "EXT", "ACC", 400.0),
            make_txn("2024-03-05", "EXT", "ACC", 200.0),
        ];
        let balances = compute_monthly_balances(&reference, &txns).unwrap();
        // cumsum[2024-02] = 400, cumsum[2024-03] = 600
        // cum_at_ref = 600 (ref_month = 2024-03, which is in the map)
        // balance[2024-02] = 1000 + (400 - 600) = 800
        assert_eq!(balances["2024-02"], 800.0);
    }

    #[test]
    fn no_transactions() {
        let reference = make_ref("ACC", "2024-06-30", 5000.0);
        let balances = compute_monthly_balances(&reference, &[]).unwrap();
        assert_eq!(balances.len(), 1);
        assert_eq!(balances["2024-06"], 5000.0);
    }

    #[test]
    fn merge_dedup() {
        let db = json!({ "balance_references": [] });
        let r1 = build_balance_reference(&BalanceReferenceInput {
            reference_id: "ref-1".to_string(),
            account_id: "ACC".to_string(),
            date: "2024-01-31".to_string(),
            amount: 1000.0,
            currency: "EUR".to_string(),
        });
        let (db2, stats1) =
            merge_balance_references_with_deduplication(db, vec![r1.clone()]).unwrap();
        assert_eq!(stats1.added, 1);
        assert_eq!(stats1.skipped, 0);

        // Second merge with the same reference_id should be skipped.
        let (_, stats2) =
            merge_balance_references_with_deduplication(db2, vec![r1]).unwrap();
        assert_eq!(stats2.added, 0);
        assert_eq!(stats2.skipped, 1);
    }
}
