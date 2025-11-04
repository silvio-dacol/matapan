use anyhow::Result;
use models::*;

/// Computes cost-of-living normalized values relative to New York using ECLI indices
pub fn compute_new_york_only(
    doc: &InputDocument,
    settings: Option<&Settings>,
    _b: &SnapshotBreakdown,
    _t: &SnapshotTotals,
    warnings: &mut Vec<String>,
) -> Result<Option<SnapshotAdjustment>> {
    // Check if New York cost-of-living normalization is enabled
    if !doc.is_ecli_enabled(settings) {
        return Ok(None);
    }

    // Get ECLI basic data and weights from document (handles both old and new formats)
    let (ecli_basic, weights) = match (doc.get_ecli_basic(), doc.get_ecli_weights(settings)) {
        (Some(basic), Some(w)) => (basic, w),
        _ => return Ok(None),
    };

    // Calculate weighted ECLI using rent, groceries, and cost of living indices
    let ecli = weights.rent_index_weight * ecli_basic.rent_index
        + weights.groceries_index_weight * ecli_basic.groceries_index
        + weights.cost_of_living_index_weight * ecli_basic.cost_of_living_index;

    // Normalize ECLI (divide by 100 unless it's effectively zero)
    let ecli_norm = if ecli.abs() < f64::EPSILON {
        1.0
    } else {
        ecli / 100.0
    };

    // Warn if ECLI normalization factor seems unusually low
    if ecli_norm < 0.2 {
        warnings.push(format!(
            "ECLI_norm very low ({:.3}) — check index values",
            ecli_norm
        ));
    }

    // Scale factor adjusts values to New York reference (higher ECLI = more expensive)
    let scale = 1.0 / ecli_norm;

    // Advantage percentage vs New York: (scale - 1) * 100
    let ny_advantage_pct = (scale - 1.0) * 100.0;

    Ok(Some(SnapshotAdjustment {
        scale,
        deflator: None,
        ecli_norm: Some(ecli_norm),
        ny_advantage_pct: Some(ny_advantage_pct),
        badge: Some(format!(
            "Relative to New York: {:+.1}% purchasing power",
            ny_advantage_pct
        )),
        normalization_applied: Some(true),
        notes: Some("Cost-of-living normalization to New York".to_string()),
    }))
}
