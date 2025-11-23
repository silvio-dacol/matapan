// Simplified types aligned with new backend dashboard schema
export interface FxRates {
  [currency: string]: number;
}

export interface DashboardMetadata {
  generated_at: string;
  settings_version: number;
  base_currency: string; // Constant (EUR) for now
}

export interface SnapshotBreakdown {
  cash: number;
  investments: number;
  pension: number; // mapped from backend "retirement"
  personal: number;
  liabilities: number;
  income: number; // monthly income (from cash_flow)
  expenses: number; // monthly expenses (from cash_flow)
}

export interface SnapshotTotals {
  assets: number;
  liabilities: number;
  net_worth: number;
  net_cash_flow: number;
}

export interface SnapshotRealWealth {
  net_worth_real: number;
  change_pct_from_prev: number;
}

export interface SnapshotPerformance {
  portfolio_nominal_return: number;
  portfolio_real_return: number;
  twr_cumulative: number;
}

export interface Snapshot {
  data_updated_at: string; // metadata.generated_at
  reference_month: string; // month
  fx_rates: FxRates;
  hicp: number;
  breakdown: SnapshotBreakdown;
  totals: SnapshotTotals;
  real_wealth: SnapshotRealWealth;
  performance: SnapshotPerformance;
}

export interface Dashboard {
  metadata: DashboardMetadata;
  snapshots: Snapshot[];
}

// API response types for specific endpoints
// Entries endpoint types (kept minimal; adjust if endpoint changes)
export interface SnapshotEntry {
  name: string;
  type: string;
  currency: string;
  balance: number;
  balance_in_base: number;
  comment: string;
}
export interface SnapshotEntriesResponse {
  date: string;
  base_currency: string;
  entries: SnapshotEntry[];
  metadata: { reference_month?: string; fx_rates?: FxRates; hicp?: number };
}
