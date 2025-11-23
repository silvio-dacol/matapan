/**
 * API client for the net-worth backend
 * Centralizes all fetch logic for dashboard endpoints
 */

import type {
  Dashboard,
  Snapshot,
  SnapshotBreakdown,
  SnapshotEntriesResponse,
  SnapshotPerformance,
  SnapshotRealWealth,
} from "./types";

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || "/api";

/**
 * Helper function to handle fetch responses
 */
async function handleResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`API Error: ${response.status} - ${errorText}`);
  }
  return response.json();
}

/**
 * GET /api/dashboard
 * Fetches the complete dashboard with all snapshots and metadata
 */
// Raw backend types (new structure) used for transformation
interface RawSnapshot {
  month: string;
  fx_rates: Record<string, number>;
  hicp: number;
  totals: { assets: number; liabilities: number; net_worth: number };
  by_category: {
    assets: Record<string, number>;
    liabilities: Record<string, number>;
  };
  cash_flow: {
    income: number;
    expenses: number;
    net_cash_flow: number;
    save_rate: number;
  };
  performance: {
    portfolio_nominal_return: number;
    portfolio_real_return: number;
    twr_cumulative: number;
  };
  real_wealth: { net_worth_real: number; change_pct_from_prev: number };
}
interface RawDashboard {
  metadata: { generated_at: string; settings_version: number };
  snapshots: RawSnapshot[];
}

function mapRawSnapshot(s: RawSnapshot, generatedAt: string): Snapshot {
  const assets = s.by_category.assets || {};
  const liabilitiesMap = s.by_category.liabilities || {};
  const breakdown: SnapshotBreakdown = {
    cash: assets.cash || 0,
    investments: assets.investments || 0,
    pension: assets.retirement || assets.pension || 0,
    personal: assets.personal || 0,
    liabilities:
      s.totals.liabilities ||
      Object.values(liabilitiesMap).reduce((a, b) => a + b, 0),
    income: s.cash_flow.income || 0,
    expenses: s.cash_flow.expenses || 0,
  };
  const real_wealth: SnapshotRealWealth = {
    net_worth_real: s.real_wealth.net_worth_real,
    change_pct_from_prev: s.real_wealth.change_pct_from_prev,
  };
  const performance: SnapshotPerformance = {
    portfolio_nominal_return: s.performance.portfolio_nominal_return,
    portfolio_real_return: s.performance.portfolio_real_return,
    twr_cumulative: s.performance.twr_cumulative,
  };
  return {
    data_updated_at: generatedAt,
    reference_month: s.month,
    fx_rates: s.fx_rates,
    hicp: s.hicp,
    breakdown,
    totals: {
      assets: s.totals.assets,
      liabilities: s.totals.liabilities,
      net_worth: s.totals.net_worth,
      net_cash_flow: s.cash_flow.net_cash_flow,
    },
    real_wealth,
    performance,
  };
}

function mapRawDashboard(raw: RawDashboard): Dashboard {
  return {
    metadata: {
      generated_at: raw.metadata.generated_at,
      settings_version: raw.metadata.settings_version,
      base_currency: "EUR",
    },
    snapshots: raw.snapshots.map((s) =>
      mapRawSnapshot(s, raw.metadata.generated_at)
    ),
  };
}

export async function getDashboard(): Promise<Dashboard> {
  const response = await fetch(`${API_BASE_URL}/dashboard`, {
    method: "GET",
    headers: { "Content-Type": "application/json" },
  });
  const raw = await handleResponse<RawDashboard>(response);
  return mapRawDashboard(raw);
}

/**
 * GET /api/dashboard/latest
 * Fetches only the latest snapshot
 */
export async function getLatestSnapshot(): Promise<Snapshot> {
  // Fetch dashboard first to get metadata (could be optimized with a separate HEAD endpoint)
  const dashboard = await getDashboard();
  const response = await fetch(`${API_BASE_URL}/dashboard/latest`, {
    method: "GET",
    headers: { "Content-Type": "application/json" },
  });
  const raw = await handleResponse<RawSnapshot>(response);
  return mapRawSnapshot(raw, dashboard.metadata.generated_at);
}

/**
 * GET /api/snapshots/:date/entries
 * Fetches detailed entries for a specific snapshot
 * @param date - The reference month in YYYY-MM format (e.g., "2025-09")
 */
export async function getSnapshotEntries(
  date: string
): Promise<SnapshotEntriesResponse> {
  const response = await fetch(`${API_BASE_URL}/snapshots/${date}/entries`, {
    method: "GET",
    headers: {
      "Content-Type": "application/json",
    },
  });
  return handleResponse<SnapshotEntriesResponse>(response);
}

/**
 * POST /api/cache/invalidate
 * Invalidates the backend cache to force data refresh
 */
export async function invalidateCache(): Promise<void> {
  const response = await fetch(`${API_BASE_URL}/cache/invalidate`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
  });

  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(
      `Failed to invalidate cache: ${response.status} - ${errorText}`
    );
  }
}
