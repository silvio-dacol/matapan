use anyhow::Result;
use models::*;

/// Computes inflation-adjusted values using HICP (Harmonized Index of Consumer Prices)
pub fn compute_inflation_only(
    doc: &InputDocument,
    b: &SnapshotBreakdown,
    _t: &SnapshotTotals,
    _warnings: &mut Vec<String>,
) -> Result<Option<SnapshotAdjustment>> {
    // Check if inflation adjustment is enabled
    let flag = doc
        .metadata
        .adjust_to_inflation
        .clone()
        .unwrap_or_else(|| "no".to_string());
    if flag.to_lowercase() != "yes" {
        return Ok(None);
    }

    // Ensure required HICP data is available
    let Some(hicp) = &doc.metadata.hicp else {
        return Ok(None);
    };
    let Some(curr_hicp) = doc.inflation.current_hicp else {
        return Ok(None);
    };

    // Calculate deflator: ratio of base HICP to current HICP
    // Deflator < 1 when prices have risen (inflation)
    let deflator = hicp.base_hicp / curr_hicp;

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
