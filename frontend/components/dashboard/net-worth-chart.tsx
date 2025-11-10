"use client";

/**
 * Net Worth Chart Component
 * Displays a line/area chart showing net worth over time
 */

import { Button } from "@/components/ui/button";
import type { Snapshot } from "@/lib/types";
import { useMemo, useState } from "react";
import {
  Area,
  AreaChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

type Timeframe = "3M" | "YTD" | "1Y" | "3Y" | "5Y" | "All";

// Mapping of timeframe to number of months (approximate for multi-year)
const timeframeMonths: Record<Exclude<Timeframe, "YTD" | "All">, number> = {
  "3M": 3,
  "1Y": 12,
  "3Y": 36,
  "5Y": 60,
};

interface NetWorthChartProps {
  snapshots: Snapshot[];
  onPercentChange?: (percent: number | null) => void; // report percent change to parent for display
}

export function NetWorthChart({
  snapshots,
  onPercentChange,
}: NetWorthChartProps) {
  const [timeframe, setTimeframe] = useState<Timeframe>("All");

  // Ensure snapshots are sorted ascending by month string (YYYY-MM)
  const sorted = useMemo(
    () =>
      [...snapshots].sort((a, b) =>
        a.reference_month.localeCompare(b.reference_month)
      ),
    [snapshots]
  );

  const filtered = useMemo(() => {
    if (sorted.length === 0) return [];
    const now = new Date();
    const currentYear = now.getFullYear().toString();

    // Handle fixed month-based timeframes with fallback when insufficient data
    if (timeframe !== "YTD" && timeframe !== "All") {
      const targetMonths = timeframeMonths[timeframe];
      // If we have fewer than requested months, just return everything available
      if (sorted.length <= targetMonths) {
        return sorted; // fallback to max available
      }
      return sorted.slice(-targetMonths);
    }

    if (timeframe === "YTD") {
      const ytd = sorted.filter((s) =>
        s.reference_month.startsWith(currentYear + "-")
      );
      // Fallback: if no data for current year yet, use last up to 12 months available
      if (ytd.length === 0) {
        const fallbackMonths = Math.min(12, sorted.length);
        return sorted.slice(-fallbackMonths);
      }
      return ytd;
    }

    // "All"
    return sorted;
  }, [sorted, timeframe]);

  // Baseline (first value in filtered timeframe) for relative scaling
  const baseline = filtered.length > 0 ? filtered[0].totals.net_worth : 0;

  const data = useMemo(
    () =>
      filtered.map((snapshot) => {
        const absolute = snapshot.totals.net_worth;
        const change = absolute - baseline; // relative difference
        const percent = baseline !== 0 ? (change / baseline) * 100 : 0;
        return {
          month: snapshot.reference_month,
          absolute,
          change,
          percent,
        };
      }),
    [filtered, baseline]
  );

  const formatCurrency = (value: number) => {
    return new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: "EUR",
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    }).format(value);
  };

  // (Axis labels hidden, month formatter removed for minimal style)

  // Percentage change between first and last point in current timeframe
  const percentChange = useMemo(() => {
    if (data.length < 2) return null;
    const first = data[0].absolute;
    const last = data[data.length - 1].absolute;
    if (first === 0) return null;
    return ((last - first) / first) * 100;
  }, [data]);

  // Notify parent when percent change updates
  useMemo(() => {
    if (onPercentChange) {
      onPercentChange(percentChange);
    }
  }, [percentChange, onPercentChange]);

  return (
    <div className="space-y-3">
      <ResponsiveContainer width="100%" height={260}>
        <AreaChart
          data={data}
          margin={{ left: 0, right: 0, top: 10, bottom: 0 }}
        >
          {/* Minimal axes: hide X axis ticks & line; Y axis only min/max (optional) */}
          <XAxis
            dataKey="month"
            tick={false}
            axisLine={false}
            tickLine={false}
            padding={{ left: 0, right: 0 }}
          />
          {/* Hide Y axis completely to eliminate reserved left space so the area starts flush */}
          <YAxis hide domain={["dataMin", "dataMax"]} />
          <Tooltip
            contentStyle={{ borderRadius: "4px" }}
            formatter={(_, __, item) => {
              const original = formatCurrency(item.payload.absolute);
              const relChange = item.payload.change;
              const relPct = item.payload.percent;
              const sign = relChange >= 0 ? "+" : "";
              return [
                `${sign}${formatCurrency(
                  Math.abs(relChange)
                )} (${sign}${relPct.toFixed(1)}%)`,
                original,
              ];
            }}
            labelFormatter={(label) => label}
          />
          <defs>
            <linearGradient id="nwGradient" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="#6366f1" stopOpacity={0.35} />
              <stop offset="100%" stopColor="#6366f1" stopOpacity={0.05} />
            </linearGradient>
          </defs>
          <Area
            type="monotone"
            dataKey="change"
            stroke="#6366f1"
            strokeWidth={3}
            fill="url(#nwGradient)"
            fillOpacity={1}
            isAnimationActive={false}
          />
        </AreaChart>
      </ResponsiveContainer>
      {/* Timeframe controls below for clarity */}
      <div
        className="flex flex-wrap gap-2 justify-center"
        aria-label="Select timeframe"
      >
        {["3M", "YTD", "1Y", "3Y", "5Y", "All"].map((tf) => (
          <Button
            key={tf}
            variant={timeframe === tf ? "default" : "outline"}
            size="sm"
            onClick={() => setTimeframe(tf as typeof timeframe)}
            aria-pressed={timeframe === tf}
          >
            {tf}
          </Button>
        ))}
      </div>
    </div>
  );
}
