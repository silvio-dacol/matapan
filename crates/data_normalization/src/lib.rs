pub mod ecli;
pub mod hicp;

use anyhow::Result;
use models::*;

/// Computes the appropriate normalization adjustments based on document settings
/// Returns (inflation_adjusted, new_york_normalized, real_purchasing_power)
pub fn compute_adjustments(
    doc: &InputDocument,
    breakdown: &SnapshotBreakdown,
    totals: &SnapshotTotals,
    warnings: &mut Vec<String>,
) -> Result<(
    Option<SnapshotAdjustment>,
    Option<SnapshotAdjustment>,
    Option<SnapshotAdjustment>,
)> {
    // Check what adjustments are enabled
    let inflation_enabled = doc
        .metadata
        .adjust_to_inflation
        .as_ref()
        .map(|s| s.to_lowercase() == "yes")
        .unwrap_or(false);

    let ny_enabled = doc
        .metadata
        .normalize_to_new_york_ecli
        .as_ref()
        .map(|s| s.to_lowercase() == "yes")
        .unwrap_or(false);

    // Compute individual adjustments based on what's enabled
    let inflation_adjusted = if inflation_enabled {
        hicp::compute_inflation_only(doc, breakdown, totals, warnings)?
    } else {
        None
    };

    let new_york_normalized = if ny_enabled {
        ecli::compute_new_york_only(doc, breakdown, totals, warnings)?
    } else {
        None
    };

    // Compute real purchasing power only if BOTH are enabled and successful
    let real_purchasing_power = if inflation_enabled && ny_enabled {
        match (&inflation_adjusted, &new_york_normalized) {
            (Some(infl), Some(ny)) => compute_combined_adjustment(breakdown, infl, ny, warnings)?,
            _ => None,
        }
    } else {
        None
    };

    Ok((
        inflation_adjusted,
        new_york_normalized,
        real_purchasing_power,
    ))
}

/// Helper function to combine inflation and NY adjustments
fn compute_combined_adjustment(
    original_breakdown: &SnapshotBreakdown,
    inflation_adj: &SnapshotAdjustment,
    ny_adj: &SnapshotAdjustment,
    warnings: &mut Vec<String>,
) -> Result<Option<SnapshotAdjustment>> {
    let deflator = inflation_adj.deflator.unwrap_or(1.0);
    let ecli_norm = ny_adj.ecli_norm.unwrap_or(1.0);

    // Combined scale: inflation deflation divided by cost-of-living normalization
    let scale = deflator / ecli_norm;

    // Warn if the combined scale seems unusually large
    if scale > 5.0 {
        warnings.push(format!(
            "Real purchasing power scale unusually large ({:.2})",
            scale
        ));
    }

    // Apply combined adjustment to all categories
    let nb = SnapshotBreakdown {
        cash: original_breakdown.cash * scale,
        investments: original_breakdown.investments * scale,
        personal: original_breakdown.personal * scale,
        pension: original_breakdown.pension * scale,
        liabilities: original_breakdown.liabilities * scale,
    };

    // Recalculate totals with adjusted values
    let assets_adj = nb.cash + nb.investments + nb.personal + nb.pension;
    let nt = SnapshotTotals {
        assets: assets_adj,
        liabilities: nb.liabilities,
        net_worth: assets_adj - nb.liabilities,
    };

    Ok(Some(SnapshotAdjustment {
        breakdown: nb,
        totals: nt,
        scale,
        deflator: Some(deflator),
        ecli_norm: Some(ecli_norm),
        normalization_applied: Some(true),
        notes: Some(
            "Combined inflation deflation and New York cost-of-living normalization".to_string(),
        ),
    }))
}
