pub mod ecli;
pub mod hicp;

use anyhow::Result;
use models::*;

/// Computes the appropriate normalization adjustments based on document settings
/// Returns (inflation_adjusted, real_purchasing_power)
pub fn compute_adjustments(
    doc: &InputDocument,
    settings: Option<&Settings>,
    totals: &SnapshotTotals,
    breakdown: &SnapshotBreakdown,
    warnings: &mut Vec<String>,
) -> Result<(Option<SnapshotAdjustment>, Option<SnapshotAdjustment>)> {
    // Check what adjustments are enabled using document and settings
    let inflation_enabled = doc.is_inflation_enabled(settings);
    let ny_enabled = doc.is_ecli_enabled(settings);

    // Compute individual adjustments based on what's enabled
    let inflation_adjusted = if inflation_enabled {
        hicp::compute_inflation_only(doc, settings, breakdown, totals, warnings)?
    } else {
        None
    };

    // Compute real purchasing power only if BOTH are enabled and successful
    let real_purchasing_power = if inflation_enabled && ny_enabled {
        // We need to compute NY data for the combined adjustment
        let ny_temp = ecli::compute_new_york_only(doc, settings, breakdown, totals, warnings)?;
        match (&inflation_adjusted, &ny_temp) {
            (Some(infl), Some(ny)) => compute_combined_adjustment(breakdown, infl, ny, warnings)?,
            _ => None,
        }
    } else {
        None
    };

    Ok((inflation_adjusted, real_purchasing_power))
}

/// Helper function to combine inflation and NY adjustments
fn compute_combined_adjustment(
    _original_breakdown: &SnapshotBreakdown,
    inflation_adj: &SnapshotAdjustment,
    ny_adj: &SnapshotAdjustment,
    warnings: &mut Vec<String>,
) -> Result<Option<SnapshotAdjustment>> {
    // Extract deflator and ecli_norm, using scale as fallback
    let deflator = inflation_adj.deflator.unwrap_or(inflation_adj.scale);
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

    Ok(Some(SnapshotAdjustment {
        scale,
        deflator: Some(deflator),
        ecli_norm: Some(ecli_norm),
        ny_advantage_pct: Some((1.0 / ecli_norm - 1.0) * 100.0),
        notes: Some(
            "Combined inflation deflation and New York cost-of-living normalization".to_string(),
        ),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a basic test breakdown
    fn test_breakdown() -> SnapshotBreakdown {
        SnapshotBreakdown {
            cash: 10000.0,
            investments: 20000.0,
            personal: 5000.0,
            pension: 15000.0,
            liabilities: 2000.0,
            positive_cash_flow: 0.0,
            negative_cash_flow: 0.0,
        }
    }

    #[test]
    fn test_compute_combined_adjustment_normal_scale() {
        let breakdown = test_breakdown();
        let mut warnings = Vec::new();

        // Mock inflation adjustment (deflator = 0.9)
        let inflation_adj = SnapshotAdjustment {
            scale: 0.9,
            deflator: Some(0.9),
            ecli_norm: None,
            ny_advantage_pct: None,
            notes: None,
        };

        // Mock NY adjustment (ecli_norm = 0.3)
        let ny_adj = SnapshotAdjustment {
            scale: 3.33,
            deflator: None,
            ecli_norm: Some(0.3),
            ny_advantage_pct: Some(233.0),
            notes: None,
        };

        let result =
            compute_combined_adjustment(&breakdown, &inflation_adj, &ny_adj, &mut warnings)
                .unwrap();

        assert!(result.is_some());
        let adj = result.unwrap();

        // Combined scale should be deflator / ecli_norm = 0.9 / 0.3 = 3.0
        assert!((adj.scale - 3.0).abs() < 0.01);
        assert_eq!(adj.deflator, Some(0.9));
        assert_eq!(adj.ecli_norm, Some(0.3));
        assert!(adj.notes.is_some());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_compute_combined_adjustment_warns_on_high_scale() {
        let breakdown = test_breakdown();
        let mut warnings = Vec::new();

        // Create scenario with very high combined scale
        let inflation_adj = SnapshotAdjustment {
            scale: 1.0,
            deflator: Some(1.0),
            ecli_norm: None,
            ny_advantage_pct: None,
            notes: None,
        };

        let ny_adj = SnapshotAdjustment {
            scale: 10.0,
            deflator: None,
            ecli_norm: Some(0.1),
            ny_advantage_pct: None,
            notes: None,
        };

        let result =
            compute_combined_adjustment(&breakdown, &inflation_adj, &ny_adj, &mut warnings)
                .unwrap();

        assert!(result.is_some());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("unusually large"));
    }

    #[test]
    fn test_compute_combined_adjustment_calculates_ny_advantage() {
        let breakdown = test_breakdown();
        let mut warnings = Vec::new();

        let inflation_adj = SnapshotAdjustment {
            scale: 1.0,
            deflator: Some(1.0),
            ecli_norm: None,
            ny_advantage_pct: None,
            notes: None,
        };

        let ny_adj = SnapshotAdjustment {
            scale: 4.0,
            deflator: None,
            ecli_norm: Some(0.25),
            ny_advantage_pct: None,
            notes: None,
        };

        let result =
            compute_combined_adjustment(&breakdown, &inflation_adj, &ny_adj, &mut warnings)
                .unwrap();

        let adj = result.unwrap();

        // NY advantage should be (1/0.25 - 1) * 100 = 300%
        assert!((adj.ny_advantage_pct.unwrap() - 300.0).abs() < 0.1);
        assert_eq!(adj.ecli_norm, Some(0.25));
        assert!(adj.notes.is_some());
    }

    #[test]
    fn test_compute_combined_adjustment_metadata() {
        let breakdown = test_breakdown();
        let mut warnings = Vec::new();

        let inflation_adj = SnapshotAdjustment {
            scale: 0.8,
            deflator: Some(0.8),
            ecli_norm: None,
            ny_advantage_pct: None,
            notes: None,
        };

        let ny_adj = SnapshotAdjustment {
            scale: 2.5,
            deflator: None,
            ecli_norm: Some(0.4),
            ny_advantage_pct: None,
            notes: None,
        };

        let result =
            compute_combined_adjustment(&breakdown, &inflation_adj, &ny_adj, &mut warnings)
                .unwrap();

        let adj = result.unwrap();

        // Verify simplified fields are set
        assert!((adj.scale - 2.0).abs() < 0.01); // 0.8 / 0.4 = 2.0
        assert_eq!(adj.deflator, Some(0.8));
        assert_eq!(adj.ecli_norm, Some(0.4));
        assert!(adj.ny_advantage_pct.is_some());
        assert!(adj.notes.is_some());
    }
}
