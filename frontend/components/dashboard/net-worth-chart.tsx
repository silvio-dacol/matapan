"use client";

import { Button } from "@/components/ui/button";
import type { Snapshot } from "@/lib/types";
import { useEffect, useMemo, useState } from "react";
import {
  Area,
  AreaChart,
  Legend,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

type Timeframe = "3M" | "YTD" | "1Y" | "3Y" | "5Y" | "All";
type ViewMode = "absolute" | "performance";

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
  const [viewMode, setViewMode] = useState<ViewMode>("absolute");

  const sortedPrimary = useMemo(
    () =>
      [...snapshots].sort((a, b) =>
        a.reference_month.localeCompare(b.reference_month)
      ),
    [snapshots]
  );
  // Single series only (inflation-adjusted comparison removed)

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
        s.reference_month.startsWith(currentYear + "-")
      );
      if (ytd.length === 0) {
        const fallbackMonths = Math.min(12, sortedPrimary.length);
        return sortedPrimary.slice(-fallbackMonths);
      }
      return ytd;
    }
    return sortedPrimary;
  }, [sortedPrimary, timeframe]);

  const filteredComparison = undefined;

  const baselinePrimary =
    filteredPrimary.length > 0 ? filteredPrimary[0].totals.net_worth : 0;
  const baselineComparison = 0;

  const comparisonMap = new Map<string, Snapshot>();

  const data = useMemo(
    () =>
      filteredPrimary.map((p, index) => {
        const primaryAbsolute = p.totals.net_worth;
        const primaryChange = primaryAbsolute - baselinePrimary;

        // Calculate percentage changes from baseline (only for net worth in performance mode)
        const primaryPerf =
          baselinePrimary !== 0
            ? ((primaryAbsolute - baselinePrimary) / baselinePrimary) * 100
            : 0;

        return {
          month: p.reference_month,
          primaryAbsolute,
          primaryChange,
          primaryPerf,
          // Absolute values for stacked area chart
          cash: p.breakdown.cash,
          investments: p.breakdown.investments,
          pension: p.breakdown.pension,
          personal: p.breakdown.personal,
        };
      }),
    [filteredPrimary, baselinePrimary]
  );

  const percentChange = useMemo(() => {
    if (data.length < 2) return null;
    const first = data[0].primaryAbsolute;
    const last = data[data.length - 1].primaryAbsolute;
    if (first === 0) return null;
    return ((last - first) / first) * 100;
  }, [data]);

  useEffect(() => {
    onPercentChange?.(percentChange);
  }, [percentChange, onPercentChange]);

  // Display plain numbers without currency symbol (base currency shown elsewhere in UI)
  const formatCurrency = (value: number) =>
    new Intl.NumberFormat("en-US", {
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    }).format(value);

  const formatPercent = (value: number) =>
    `${value >= 0 ? "+" : ""}${value.toFixed(1)}%`;

  return (
    <div className="space-y-3">
      {/* View Mode Toggle */}
      <div className="flex justify-center gap-2 pb-2">
        <Button
          variant={viewMode === "absolute" ? "default" : "outline"}
          size="sm"
          onClick={() => setViewMode("absolute")}
        >
          Absolute Values
        </Button>
        <Button
          variant={viewMode === "performance" ? "default" : "outline"}
          size="sm"
          onClick={() => setViewMode("performance")}
        >
          Performance %
        </Button>
      </div>
      <ResponsiveContainer width="100%" height={380}>
        <AreaChart
          data={data}
          margin={{ left: 0, right: 0, top: 10, bottom: 0 }}
        >
          <XAxis
            dataKey="month"
            tick={false}
            axisLine={false}
            tickLine={false}
            padding={{ left: 0, right: 0 }}
          />
          <YAxis hide domain={["dataMin", "dataMax"]} />
          <Tooltip
            content={({ active, payload, label }) => {
              if (!active || !payload || payload.length === 0) return null;
              
              if (viewMode === "performance") {
                const data = payload[0].payload as {
                  primaryAbsolute: number;
                  primaryPerf: number;
                };
                return (
                  <div className="rounded-md border bg-background/95 backdrop-blur px-3 py-2 shadow-lg min-w-[200px] space-y-2">
                    <div className="text-xs font-medium tracking-wide text-muted-foreground">
                      {label}
                    </div>
                    <div className="space-y-1">
                      <div className="flex items-baseline justify-between gap-3">
                        <span className="text-[11px] uppercase text-muted-foreground">
                          Net Worth
                        </span>
                        <span className="font-semibold text-sm tabular-nums text-indigo-600">
                          {formatPercent(data.primaryPerf)}
                        </span>
                      </div>
                      <div className="flex items-baseline justify-between gap-3 pt-1 border-t">
                        <span className="text-[11px] uppercase text-muted-foreground">
                          Absolute
                        </span>
                        <span className="font-medium text-xs tabular-nums text-muted-foreground">
                          {formatCurrency(data.primaryAbsolute)}
                        </span>
                      </div>
                    </div>
                  </div>
                );
              } else {
                // Absolute mode - show stacked values
                const data = payload[0].payload as {
                  primaryAbsolute: number;
                  cash: number;
                  investments: number;
                  pension: number;
                  personal: number;
                };
                return (
                  <div className="rounded-md border bg-background/95 backdrop-blur px-3 py-2 shadow-lg min-w-[200px] space-y-2">
                    <div className="text-xs font-medium tracking-wide text-muted-foreground">
                      {label}
                    </div>
                    <div className="space-y-1">
                      <div className="flex items-baseline justify-between gap-3">
                        <span className="text-[11px] uppercase text-muted-foreground">
                          Net Worth
                        </span>
                        <span className="font-semibold text-sm tabular-nums text-indigo-600">
                          {formatCurrency(data.primaryAbsolute)}
                        </span>
                      </div>
                      <div className="border-t pt-1 mt-1 space-y-1">
                        <div className="flex items-baseline justify-between gap-3">
                          <span className="text-[11px] uppercase text-muted-foreground">
                            Cash
                          </span>
                          <span className="font-semibold text-xs tabular-nums text-emerald-600">
                            {formatCurrency(data.cash)}
                          </span>
                        </div>
                        <div className="flex items-baseline justify-between gap-3">
                          <span className="text-[11px] uppercase text-muted-foreground">
                            Investments
                          </span>
                          <span className="font-semibold text-xs tabular-nums text-blue-600">
                            {formatCurrency(data.investments)}
                          </span>
                        </div>
                        <div className="flex items-baseline justify-between gap-3">
                          <span className="text-[11px] uppercase text-muted-foreground">
                            Pension
                          </span>
                          <span className="font-semibold text-xs tabular-nums text-purple-600">
                            {formatCurrency(data.pension)}
                          </span>
                        </div>
                        <div className="flex items-baseline justify-between gap-3">
                          <span className="text-[11px] uppercase text-muted-foreground">
                            Personal
                          </span>
                          <span className="font-semibold text-xs tabular-nums text-amber-600">
                            {formatCurrency(data.personal)}
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>
                );
              }
            }}
          />
          <defs>
            <linearGradient id="nwGradientNominal" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="#6366f1" stopOpacity={0.18} />
              <stop offset="100%" stopColor="#6366f1" stopOpacity={0.06} />
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
            // Performance mode: only show Net Worth percentage change
            <Area
              type="monotone"
              dataKey="primaryPerf"
              stroke="#6366f1"
              strokeWidth={3}
              fill="url(#nwGradientNominal)"
              fillOpacity={1}
              isAnimationActive={false}
              name="Net Worth"
            />
          ) : (
            // Absolute mode: stacked area chart showing asset composition
            <>
              <Area
                type="monotone"
                dataKey="cash"
                stackId="1"
                stroke="#10b981"
                strokeWidth={1.5}
                fill="url(#cashGradient)"
                fillOpacity={1}
                isAnimationActive={false}
                name="Cash"
              />
              <Area
                type="monotone"
                dataKey="investments"
                stackId="1"
                stroke="#3b82f6"
                strokeWidth={1.5}
                fill="url(#investmentsGradient)"
                fillOpacity={1}
                isAnimationActive={false}
                name="Investments"
              />
              <Area
                type="monotone"
                dataKey="pension"
                stackId="1"
                stroke="#a855f7"
                strokeWidth={1.5}
                fill="url(#pensionGradient)"
                fillOpacity={1}
                isAnimationActive={false}
                name="Pension"
              />
              <Area
                type="monotone"
                dataKey="personal"
                stackId="1"
                stroke="#f59e0b"
                strokeWidth={1.5}
                fill="url(#personalGradient)"
                fillOpacity={1}
                isAnimationActive={false}
                name="Personal"
              />
            </>
          )}
          <Legend
            verticalAlign="top"
            height={36}
            iconType="line"
            wrapperStyle={{ paddingBottom: "10px" }}
          />
        </AreaChart>
      </ResponsiveContainer>
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
