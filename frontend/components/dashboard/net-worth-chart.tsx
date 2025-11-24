"use client";

import { Button } from "@/components/ui/button";
import type { Snapshot } from "@/lib/types";
import { useEffect, useMemo, useState } from "react";
import {
  Area,
  AreaChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

type Timeframe = "3M" | "YTD" | "1Y" | "3Y" | "5Y" | "All";
type ViewMode = "value" | "performance";

const timeframeMonths: Record<Exclude<Timeframe, "YTD" | "All">, number> = {
  "3M": 3,
  "1Y": 12,
  "3Y": 36,
  "5Y": 60,
};

interface NetWorthChartProps {
  snapshots: Snapshot[];
  onPercentChange?: (percent: number | null) => void;
}

export function NetWorthChart({
  snapshots,
  onPercentChange,
}: NetWorthChartProps) {
  const [timeframe, setTimeframe] = useState<Timeframe>("YTD");
  const [viewMode, setViewMode] = useState<ViewMode>("value");

  const getMonthKey = (s: Snapshot) => s.reference_month;

  const sortedPrimary = useMemo(
    () =>
      [...snapshots].sort((a, b) =>
        getMonthKey(a).localeCompare(getMonthKey(b))
      ),
    [snapshots]
  );

  const filteredPrimary = useMemo(() => {
    if (sortedPrimary.length === 0) return [];

    const now = new Date();
    const currentYear = now.getFullYear().toString();

    if (timeframe !== "YTD" && timeframe !== "All") {
      const targetMonths =
        timeframeMonths[timeframe as Exclude<Timeframe, "YTD" | "All">];
      return sortedPrimary.length <= targetMonths
        ? sortedPrimary
        : sortedPrimary.slice(-targetMonths);
    }

    if (timeframe === "YTD") {
      const ytd = sortedPrimary.filter((s) =>
        getMonthKey(s).startsWith(currentYear + "-")
      );
      if (ytd.length === 0) {
        const fallbackMonths = Math.min(12, sortedPrimary.length);
        return sortedPrimary.slice(-fallbackMonths);
      }
      return ytd;
    }

    return sortedPrimary;
  }, [sortedPrimary, timeframe]);

  const baselinePrimary =
    filteredPrimary.length > 0 ? filteredPrimary[0].totals.net_worth : 0;

  const data = useMemo(() => {
    const twrBaseline =
      filteredPrimary.length > 0
        ? filteredPrimary[0].performance.twr_cumulative
        : 1;

    return filteredPrimary.map((p) => {
      const month = getMonthKey(p);
      const totals = p.totals;
      const performance = p.performance;
      const breakdown = p.breakdown;

      const primaryAbsolute = totals.net_worth;

      const primaryPercentChange =
        baselinePrimary !== 0
          ? ((primaryAbsolute - baselinePrimary) / baselinePrimary) * 100
          : 0;

      const twr = performance.twr_cumulative;
      const perfPct = (twr / twrBaseline - 1) * 100;

      const twrCumulative = performance.twr_cumulative;

      return {
        month,
        primaryAbsolute,
        primaryPercentChange,
        performance: perfPct,
        twrCumulative,
        cash: breakdown.cash,
        investments: breakdown.investments,
        pension: breakdown.pension,
        personal: breakdown.personal,
      };
    });
  }, [filteredPrimary, baselinePrimary]);

  const percentChange = useMemo(() => {
    if (data.length < 2) return null;
    const first = data[0].primaryAbsolute;
    const last = data[data.length - 1].primaryAbsolute;
    if (!first) return null;
    return ((last - first) / first) * 100;
  }, [data]);

  useEffect(() => {
    onPercentChange?.(percentChange);
  }, [percentChange, onPercentChange]);

  const formatCurrency = (value: number) =>
    new Intl.NumberFormat("en-US", {
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    }).format(value);

  const formatPercent = (value: number) =>
    `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`;

  const categories = useMemo(() => {
    // Use the known breakdown categories from SnapshotBreakdown
    return ["cash", "investments", "pension", "personal"];
  }, []);

  const categoryColors: Record<string, string> = {
    cash: "#10b981",
    investments: "#3b82f6",
    pension: "#a855f7",
    personal: "#f59e0b",
  };

  const categoryFills: Record<string, string> = {
    cash: "url(#cashGradient)",
    investments: "url(#investmentsGradient)",
    pension: "url(#pensionGradient)",
    personal: "url(#personalGradient)",
  };

  return (
    <div className="space-y-3">
      <div className="relative">
        {/* top right toggle */}
        <div className="absolute right-3 top-3 z-10 flex gap-2">
          <Button
            variant={viewMode === "value" ? "default" : "outline"}
            size="sm"
            onClick={() => setViewMode("value")}
          >
            Value
          </Button>
          <Button
            variant={viewMode === "performance" ? "default" : "outline"}
            size="sm"
            onClick={() => setViewMode("performance")}
          >
            Performance
          </Button>
        </div>

        <ResponsiveContainer width="100%" height={380}>
          <AreaChart
            data={data}
            margin={{ left: 0, right: 0, top: 10, bottom: 0 }}
          >
            {/* hide x axis visuals but keep data for tooltip */}
            <XAxis dataKey="month" hide padding={{ left: 0, right: 0 }} />
            <YAxis
              hide
              domain={[
                (dataMin: number) => Math.min(dataMin - 1, -5),
                (dataMax: number) => Math.max(dataMax + 1, 5),
              ]}
            />

            <Tooltip
              content={({ active, payload, label }) => {
                if (!active || !payload || payload.length === 0) return null;
                const row = payload[0].payload as {
                  month: string;
                  primaryAbsolute: number;
                  primaryPercentChange: number;
                  performance: number;
                  twrCumulative: number;
                  cash: number;
                  investments: number;
                  pension: number;
                  personal: number;
                };

                if (viewMode === "performance") {
                  return (
                    <div className="rounded-md border bg-background/95 backdrop-blur px-3 py-2 shadow-lg min-w-[200px] space-y-2">
                      <div className="text-xs font-medium tracking-wide text-muted-foreground">
                        {label}
                      </div>
                      <div className="space-y-1">
                        <div className="flex items-baseline justify-between gap-3">
                          <span className="text-[11px] uppercase text-muted-foreground">
                            Real performance
                          </span>
                          <span className="font-semibold text-sm tabular-nums text-emerald-600">
                            {formatPercent(row.performance)}
                          </span>
                        </div>
                        <div className="flex items-baseline justify-between gap-3 pt-1 border-t">
                          <span className="text-[11px] uppercase text-muted-foreground">
                            TWR factor
                          </span>
                          <span className="font-medium text-xs tabular-nums text-muted-foreground">
                            {row.twrCumulative?.toFixed(4)}
                          </span>
                        </div>
                        <div className="text-[10px] text-muted-foreground pt-1">
                          Time weighted return ignoring deposits and withdrawals
                        </div>
                      </div>
                    </div>
                  );
                }

                return (
                  <div className="rounded-md border bg-background/95 backdrop-blur px-3 py-2 shadow-lg min-w-[220px] space-y-2">
                    <div className="text-xs font-medium tracking-wide text-muted-foreground">
                      {label}
                    </div>
                    <div className="space-y-1">
                      <div className="flex items-baseline justify-between gap-3">
                        <span className="text-[11px] uppercase text-muted-foreground">
                          Net worth
                        </span>
                        <span className="font-semibold text-sm tabular-nums text-indigo-600">
                          {formatCurrency(row.primaryAbsolute)}
                        </span>
                      </div>
                      <div className="flex items-baseline justify-between gap-3">
                        <span className="text-[11px] uppercase text-muted-foreground">
                          Change
                        </span>
                        <span className="font-semibold text-xs tabular-nums text-indigo-600">
                          {formatPercent(row.primaryPercentChange)}
                        </span>
                      </div>
                      <div className="border-t pt-1 mt-1 space-y-1">
                        <div className="flex items-baseline justify-between gap-3">
                          <span className="text-[11px] uppercase text-muted-foreground">
                            Cash
                          </span>
                          <span className="font-semibold text-xs tabular-nums text-emerald-600">
                            {formatCurrency(row.cash)}
                          </span>
                        </div>
                        <div className="flex items-baseline justify-between gap-3">
                          <span className="text-[11px] uppercase text-muted-foreground">
                            Investments
                          </span>
                          <span className="font-semibold text-xs tabular-nums text-blue-600">
                            {formatCurrency(row.investments)}
                          </span>
                        </div>
                        <div className="flex items-baseline justify-between gap-3">
                          <span className="text-[11px] uppercase text-muted-foreground">
                            Pension
                          </span>
                          <span className="font-semibold text-xs tabular-nums text-purple-600">
                            {formatCurrency(row.pension)}
                          </span>
                        </div>
                        <div className="flex items-baseline justify-between gap-3">
                          <span className="text-[11px] uppercase text-muted-foreground">
                            Personal
                          </span>
                          <span className="font-semibold text-xs tabular-nums text-amber-600">
                            {formatCurrency(row.personal)}
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>
                );
              }}
            />

            <defs>
              <linearGradient id="nwGradientTotal" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="#6366f1" stopOpacity={0.2} />
                <stop offset="100%" stopColor="#6366f1" stopOpacity={0.05} />
              </linearGradient>
              <linearGradient id="cashGradient" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="#10b981" stopOpacity={0.15} />
                <stop offset="100%" stopColor="#10b981" stopOpacity={0.05} />
              </linearGradient>
              <linearGradient
                id="investmentsGradient"
                x1="0"
                y1="0"
                x2="0"
                y2="1"
              >
                <stop offset="0%" stopColor="#3b82f6" stopOpacity={0.15} />
                <stop offset="100%" stopColor="#3b82f6" stopOpacity={0.05} />
              </linearGradient>
              <linearGradient id="pensionGradient" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="#a855f7" stopOpacity={0.15} />
                <stop offset="100%" stopColor="#a855f7" stopOpacity={0.05} />
              </linearGradient>
              <linearGradient id="personalGradient" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="#f59e0b" stopOpacity={0.15} />
                <stop offset="100%" stopColor="#f59e0b" stopOpacity={0.05} />
              </linearGradient>
            </defs>

            {viewMode === "performance" ? (
              <Area
                type="monotone"
                dataKey="performance"
                stroke="#22c55e"
                strokeWidth={3}
                fill="none"
                isAnimationActive={false}
                name="Real performance %"
              />
            ) : (
              <>
                {categories.map((cat) => (
                  <Area
                    key={cat}
                    type="monotone"
                    dataKey={cat}
                    stackId="1"
                    stroke={categoryColors[cat] ?? "#888"}
                    fill={categoryFills[cat] ?? "rgba(0,0,0,0.1)"}
                    strokeWidth={1.5}
                    isAnimationActive={false}
                  />
                ))}
              </>
            )}
          </AreaChart>
        </ResponsiveContainer>
      </div>

      <div
        className="flex flex-wrap gap-2 justify-center"
        aria-label="Select timeframe"
      >
        {["3M", "YTD", "1Y", "3Y", "5Y", "All"].map((tf) => (
          <Button
            key={tf}
            variant={timeframe === tf ? "default" : "outline"}
            size="sm"
            onClick={() => setTimeframe(tf as Timeframe)}
            aria-pressed={timeframe === tf}
          >
            {tf}
          </Button>
        ))}
      </div>
    </div>
  );
}
