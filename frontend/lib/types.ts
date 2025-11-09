/**
 * TypeScript types mirroring the Rust backend API structures
 */

export interface FxRates {
  [currency: string]: number;
}

export interface HICP {
  base_year: string;
  base_month: string;
  base_hicp: number;
}

export interface ECLI {
  rent_index_weight: number;
  groceries_index_weight: number;
  cost_of_living_index_weight: number;
  restaurant_price_index_weight: number;
  local_purchasing_power_index_weight: number;
}

export interface ECLIData {
  rent_index: number;
  groceries_index: number;
  cost_of_living_index: number;
}

export interface Categories {
  assets: string[];
  liabilities: string[];
}

export interface Metadata {
  generated_at: string;
  base_currency: string;
  normalize: string;
  hicp: HICP;
  ecli: ECLI;
  categories: Categories;
}

export interface SnapshotBreakdown {
  cash: number;
  investments: number;
  personal: number;
  pension: number;
  liabilities: number;
}

export interface SnapshotTotals {
  assets: number;
  liabilities: number;
  net_worth: number;
}

export interface InflationAdjusted {
  scale: number;
  deflator: number;
  notes: string;
}

export interface RealPurchasingPower {
  scale: number;
  deflator: number;
  ecli_norm: number;
  ny_advantage_pct: number;
  badge: string;
  normalization_applied: boolean;
  notes: string;
}

export interface Snapshot {
  data_updated_at: string;
  reference_month: string;
  fx_rates: FxRates;
  hicp: number;
  ecli: ECLIData;
  breakdown: SnapshotBreakdown;
  totals: SnapshotTotals;
  inflation_adjusted: InflationAdjusted;
  real_purchasing_power: RealPurchasingPower;
}

export interface Dashboard {
  metadata: Metadata;
  snapshots: Snapshot[];
  latest: Snapshot;
}

// API response types for specific endpoints
export interface SnapshotEntry {
  category: string;
  sub_category: string;
  description: string;
  original_amount: number;
  original_currency: string;
  converted_amount_eur: number;
  notes?: string;
}

export interface SnapshotEntriesResponse {
  reference_month: string;
  entries: SnapshotEntry[];
}
