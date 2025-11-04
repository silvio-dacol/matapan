// Auto-refresh with Polling
// Simple approach: Check for updates every N seconds

import { useEffect, useState } from "react";

interface Dashboard {
  generated_at: string;
  // ... other fields
}

export function useAutoRefreshDashboard(intervalMs: number = 5000) {
  const [dashboard, setDashboard] = useState<Dashboard | null>(null);
  const [lastUpdate, setLastUpdate] = useState<string>("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchDashboard = async () => {
      try {
        const response = await fetch("http://localhost:3000/api/dashboard");
        if (!response.ok) throw new Error("Failed to fetch");

        const data: Dashboard = await response.json();

        // Only update if data has changed
        if (data.generated_at !== lastUpdate) {
          console.log("📊 Dashboard updated!", data.generated_at);
          setDashboard(data);
          setLastUpdate(data.generated_at);
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : "Unknown error");
      }
    };

    // Initial fetch
    fetchDashboard();

    // Poll every intervalMs
    const intervalId = setInterval(fetchDashboard, intervalMs);

    return () => clearInterval(intervalId);
  }, [intervalMs, lastUpdate]);

  return { dashboard, error, lastUpdate };
}

// Usage in component
export function AutoRefreshDashboard() {
  const { dashboard, error, lastUpdate } = useAutoRefreshDashboard(5000); // Check every 5 seconds

  if (error) return <div>Error: {error}</div>;
  if (!dashboard) return <div>Loading...</div>;

  return (
    <div>
      <div className="update-indicator">
        Last updated: {new Date(lastUpdate).toLocaleString()}
      </div>
      {/* Your charts and tables here */}
    </div>
  );
}
