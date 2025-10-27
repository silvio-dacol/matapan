use anyhow::Result;
use models::*;

/// Computes inflation-adjusted values using HICP (Harmonized Index of Consumer Prices)
pub fn compute_inflation_only(
    doc: &InputDocument,
    settings: Option<&Settings>,
    b: &SnapshotBreakdown,
    _t: &SnapshotTotals,
    _warnings: &mut Vec<String>,
) -> Result<Option<SnapshotAdjustment>> {
    // Check if inflation adjustment is enabled
    if !doc.is_inflation_enabled(settings) {
        return Ok(None);
    }

    // Get HICP base value from document or settings
    let Some(base_hicp) = doc.get_hicp_base(settings) else {
        return Ok(None);
    };

    // Get current HICP from document
    let Some(curr_hicp) = doc.get_current_hicp() else {
        return Ok(None);
    };

    // Calculate deflator: ratio of base HICP to current HICP
    // Deflator < 1 when prices have risen (inflation)
    let deflator = base_hicp / curr_hicp;

    let scale = deflator;

    // Apply inflation adjustment to all categories
    let nb = SnapshotBreakdown {
        cash: b.cash * scale,
        investments: b.investments * scale,
        personal: b.personal * scale,
        pension: b.pension * scale,
        liabilities: b.liabilities * scale,
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
        ecli_norm: None,
        normalization_applied: None,
        notes: Some("Inflation-only deflation using HICP".to_string()),
    }))
}
