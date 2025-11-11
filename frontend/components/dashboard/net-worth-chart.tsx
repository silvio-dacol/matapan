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

const timeframeMonths: Record<Exclude<Timeframe, "YTD" | "All">, number> = {
  "3M": 3,
  "1Y": 12,
  "3Y": 36,
  "5Y": 60,
};

interface NetWorthChartProps {
  snapshots: Snapshot[]; // primary series (nominal if toggle off, real if toggle on)
  comparisonSnapshots?: Snapshot[]; // optional nominal series when viewing inflation-adjusted
  onPercentChange?: (percent: number | null) => void;
}

export function NetWorthChart({
  snapshots,
  comparisonSnapshots,
  onPercentChange,
}: NetWorthChartProps) {
  const [timeframe, setTimeframe] = useState<Timeframe>("All");

  const sortedPrimary = useMemo(
    () =>
      [...snapshots].sort((a, b) =>
        a.reference_month.localeCompare(b.reference_month)
      ),
    [snapshots]
  );
  const sortedComparison = useMemo(
    () =>
      comparisonSnapshots
        ? [...comparisonSnapshots].sort((a, b) =>
            a.reference_month.localeCompare(b.reference_month)
          )
        : undefined,
    [comparisonSnapshots]
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

  const filteredComparison = useMemo(() => {
    if (!sortedComparison) return undefined;
    if (filteredPrimary.length === 0) return [];
    // Keep only months that appear in filteredPrimary for alignment
    const monthsSet = new Set(filteredPrimary.map((s) => s.reference_month));
    return sortedComparison.filter((s) => monthsSet.has(s.reference_month));
  }, [sortedComparison, filteredPrimary]);

  const baselinePrimary =
    filteredPrimary.length > 0 ? filteredPrimary[0].totals.net_worth : 0;
  const baselineComparison =
    filteredComparison && filteredComparison.length > 0
      ? filteredComparison[0].totals.net_worth
      : 0;

  const comparisonMap = useMemo(() => {
    if (!filteredComparison) return new Map<string, Snapshot>();
    return new Map(filteredComparison.map((s) => [s.reference_month, s]));
  }, [filteredComparison]);

  const data = useMemo(
    () =>
      filteredPrimary.map((p) => {
        const primaryAbsolute = p.totals.net_worth;
        const primaryChange = primaryAbsolute - baselinePrimary;
        let comparisonAbsolute: number | undefined;
        let comparisonChange: number | undefined;
        if (filteredComparison) {
          const comp = comparisonMap.get(p.reference_month);
          if (comp) {
            comparisonAbsolute = comp.totals.net_worth;
            comparisonChange = comparisonAbsolute - baselineComparison;
          }
        }
        return {
          month: p.reference_month,
          primaryAbsolute,
          primaryChange,
          comparisonAbsolute,
          comparisonChange,
        };
      }),
    [
      filteredPrimary,
      filteredComparison,
      baselinePrimary,
      baselineComparison,
      comparisonMap,
    ]
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

  return (
    <div className="space-y-3">
      <ResponsiveContainer width="100%" height={260}>
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
              // Both series share the same underlying payload shape
              const base = payload[0].payload as {
                primaryAbsolute: number;
                comparisonAbsolute?: number;
              };
              const primary = base.primaryAbsolute;
              const comparison = base.comparisonAbsolute;
              return (
                <div className="rounded-md border bg-background/95 backdrop-blur px-3 py-2 shadow-lg min-w-[180px] space-y-2">
                  <div className="text-xs font-medium tracking-wide text-muted-foreground">
                    {label}
                  </div>
                  <div className="space-y-1">
                    {comparison !== undefined ? (
                      <>
                        <div className="flex items-baseline justify-between gap-1">
                          <span className="text-[11px] uppercase text-muted-foreground">
                            Inflation Adj.
                          </span>
                          <span className="font-semibold text-sm tabular-nums text-emerald-600">
                            {formatCurrency(primary)}
                          </span>
                        </div>
                        <div className="flex items-baseline justify-between gap-1">
                          <span className="text-[11px] uppercase text-muted-foreground">
                            Nominal
                          </span>
                          <span className="font-semibold text-sm tabular-nums text-indigo-600">
                            {formatCurrency(comparison)}
                          </span>
                        </div>
                      </>
                    ) : (
                      <div className="flex items-baseline justify-between gap-1">
                        <span className="text-[11px] uppercase text-muted-foreground">
                          Net Worth
                        </span>
                        <span className="font-semibold text-sm tabular-nums text-indigo-600">
                          {formatCurrency(primary)}
                        </span>
                      </div>
                    )}
                  </div>
                </div>
              );
            }}
          />
          <defs>
            <linearGradient id="nwGradientNominal" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="#6366f1" stopOpacity={0.18} />
              <stop offset="100%" stopColor="#6366f1" stopOpacity={0.06} />
            </linearGradient>
          </defs>
          {filteredComparison ? (
            <>
              <Area
                type="monotone"
                dataKey="comparisonChange"
                stroke="#6366f1"
                strokeWidth={3}
                fill="url(#nwGradientNominal)"
                fillOpacity={1}
                isAnimationActive={false}
                name="Nominal"
              />
              <Area
                type="monotone"
                dataKey="primaryChange"
                stroke="#10b981"
                strokeWidth={2}
                fill="none"
                isAnimationActive={false}
                name="Inflation-adjusted"
              />
            </>
          ) : (
            <Area
              type="monotone"
              dataKey="primaryChange"
              stroke="#6366f1"
              strokeWidth={3}
              fill="url(#nwGradientNominal)"
              fillOpacity={1}
              isAnimationActive={false}
              name="Net Worth"
            />
          )}
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
