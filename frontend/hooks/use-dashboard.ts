"use client";

/**
 * React Query hooks for dashboard data
 * Provides hooks for fetching and caching dashboard data with auto-refresh
 */

import {
  getDashboard,
  getLatestSnapshot,
  getSnapshotEntries,
  invalidateCache,
} from "@/lib/api";
import type { Dashboard, Snapshot, SnapshotEntriesResponse } from "@/lib/types";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

// Query keys for cache management
export const queryKeys = {
  dashboard: ["dashboard"] as const,
  latestSnapshot: ["dashboard", "latest"] as const,
  snapshotEntries: (date: string) => ["snapshots", date, "entries"] as const,
};

/**
 * Hook to fetch the complete dashboard
 * @param refetchInterval - Optional polling interval in milliseconds (default: 30000 = 30 seconds)
 */
export function useDashboard(refetchInterval: number = 30000) {
  return useQuery<Dashboard>({
    queryKey: queryKeys.dashboard,
    queryFn: getDashboard,
    refetchInterval, // Auto-refresh every 30 seconds
    refetchIntervalInBackground: false, // Don't refresh when tab is not active
  });
}

/**
 * Hook to fetch only the latest snapshot
 * @param refetchInterval - Optional polling interval in milliseconds (default: 30000 = 30 seconds)
 */
export function useLatestSnapshot(refetchInterval: number = 30000) {
  return useQuery<Snapshot>({
    queryKey: queryKeys.latestSnapshot,
    queryFn: getLatestSnapshot,
    refetchInterval,
    refetchIntervalInBackground: false,
  });
}

/**
 * Hook to fetch detailed entries for a specific snapshot
 * @param date - The reference month in YYYY-MM format (e.g., "2025-09")
 * @param enabled - Whether to fetch the data (default: true)
 */
export function useSnapshotEntries(date: string, enabled: boolean = true) {
  return useQuery<SnapshotEntriesResponse>({
    queryKey: queryKeys.snapshotEntries(date),
    queryFn: () => getSnapshotEntries(date),
    enabled: enabled && !!date, // Only fetch if enabled and date is provided
  });
}

/**
 * Hook to invalidate the backend cache
 * Returns a mutation function that can be called to force a cache refresh
 */
export function useInvalidateCache() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: invalidateCache,
    onSuccess: () => {
      // Invalidate all dashboard-related queries to trigger a refresh
      queryClient.invalidateQueries({ queryKey: queryKeys.dashboard });
      queryClient.invalidateQueries({ queryKey: queryKeys.latestSnapshot });
    },
  });
}
