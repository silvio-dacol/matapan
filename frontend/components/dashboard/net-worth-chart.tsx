"use client";

/**
 * Net Worth Chart Component
 * Displays a line/area chart showing net worth over time
 */

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { Snapshot } from "@/lib/types";
import { useMemo, useState } from "react";
import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

interface NetWorthChartProps {
  snapshots: Snapshot[];
}

export function NetWorthChart({ snapshots }: NetWorthChartProps) {
  const [timeframe, setTimeframe] = useState<"3M" | "YTD" | "1Y" | "All">(
    "All"
  );

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

    switch (timeframe) {
      case "3M": {
        return sorted.slice(-3);
      }
      case "1Y": {
        return sorted.slice(-12);
      }
      case "YTD": {
        return sorted.filter((s) =>
          s.reference_month.startsWith(currentYear + "-")
        );
      }
      case "All":
      default:
        return sorted;
    }
  }, [sorted, timeframe]);

  const data = useMemo(
    () =>
      filtered.map((snapshot) => ({
        month: snapshot.reference_month,
        "Net Worth": snapshot.totals.net_worth,
      })),
    [filtered]
  );

  const formatCurrency = (value: number) => {
    return new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: "EUR",
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    }).format(value);
  };

  const formatMonth = (month: string) => {
    const [year, monthNum] = month.split("-");
    return `${monthNum}/${year.slice(2)}`;
  };

  // Percentage change between first and last point in current timeframe
  const percentChange = useMemo(() => {
    if (data.length < 2) return null;
    const first = data[0]["Net Worth"];
    const last = data[data.length - 1]["Net Worth"];
    if (first === 0) return null; // avoid divide by zero / meaningless percentage
    const pct = ((last - first) / first) * 100;
    return pct;
  }, [data]);

  const pctLabel =
    percentChange === null
      ? "—"
      : `${percentChange > 0 ? "+" : ""}${percentChange.toFixed(1)}%`;
  const pctColor =
    percentChange === null
      ? ""
      : percentChange >= 0
      ? "text-green-600"
      : "text-red-600";

  return (
    <div className="space-y-4">
      {/* Timeframe selector */}
      <div className="flex flex-wrap gap-2">
        {["3M", "YTD", "1Y", "All"].map((tf) => (
          <Button
            key={tf}
            variant={timeframe === tf ? "default" : "outline"}
            size="sm"
            onClick={() => setTimeframe(tf as typeof timeframe)}
          >
            {tf}
          </Button>
        ))}
        <Badge variant="secondary" className={`ml-auto ${pctColor}`}>
          {pctLabel}
        </Badge>
      </div>
      <ResponsiveContainer width="100%" height={300}>
        <AreaChart data={data}>
          <CartesianGrid strokeDasharray="3 3" />
          <XAxis
            dataKey="month"
            tickFormatter={formatMonth}
            angle={-45}
            textAnchor="end"
            height={60}
          />
          <YAxis tickFormatter={formatCurrency} />
          <Tooltip
            formatter={(value: number) => formatCurrency(Number(value))}
            labelFormatter={(label) => `Month: ${label}`}
          />
          <Area
            type="monotone"
            dataKey="Net Worth"
            stroke="#3b82f6"
            fill="#3b82f6"
            fillOpacity={0.8}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}
