/**
 * API client for the net-worth backend
 * Centralizes all fetch logic for dashboard endpoints
 */

import type { Dashboard, Snapshot, SnapshotEntriesResponse } from "./types";

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
export async function getDashboard(): Promise<Dashboard> {
  const response = await fetch(`${API_BASE_URL}/dashboard`, {
    method: "GET",
    headers: {
      "Content-Type": "application/json",
    },
  });
  return handleResponse<Dashboard>(response);
}

/**
 * GET /api/dashboard/latest
 * Fetches only the latest snapshot
 */
export async function getLatestSnapshot(): Promise<Snapshot> {
  const response = await fetch(`${API_BASE_URL}/dashboard/latest`, {
    method: "GET",
    headers: {
      "Content-Type": "application/json",
    },
  });
  return handleResponse<Snapshot>(response);
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
