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
        const cashChange =
          p.breakdown.cash - (filteredPrimary[0]?.breakdown.cash || 0);
        const investmentsChange =
          p.breakdown.investments -
          (filteredPrimary[0]?.breakdown.investments || 0);
        const pensionChange =
          p.breakdown.pension - (filteredPrimary[0]?.breakdown.pension || 0);
        const personalChange =
          p.breakdown.personal - (filteredPrimary[0]?.breakdown.personal || 0);

        // Calculate percentage changes from baseline
        const baselineCash = filteredPrimary[0]?.breakdown.cash || 0;
        const baselineInvestments =
          filteredPrimary[0]?.breakdown.investments || 0;
        const baselinePension = filteredPrimary[0]?.breakdown.pension || 0;
        const baselinePersonal = filteredPrimary[0]?.breakdown.personal || 0;

        const primaryPerf =
          baselinePrimary !== 0
            ? ((primaryAbsolute - baselinePrimary) / baselinePrimary) * 100
            : 0;
        const cashPerf =
          baselineCash !== 0
            ? ((p.breakdown.cash - baselineCash) / baselineCash) * 100
            : 0;
        const investmentsPerf =
          baselineInvestments !== 0
            ? ((p.breakdown.investments - baselineInvestments) /
                baselineInvestments) *
              100
            : 0;
        const pensionPerf =
          baselinePension !== 0
            ? ((p.breakdown.pension - baselinePension) / baselinePension) * 100
            : 0;
        const personalPerf =
          baselinePersonal !== 0
            ? ((p.breakdown.personal - baselinePersonal) / baselinePersonal) *
              100
            : 0;

        return {
          month: p.reference_month,
          primaryAbsolute,
          primaryChange,
          primaryPerf,
          cashAbsolute: p.breakdown.cash,
          cashChange,
          cashPerf,
          investmentsAbsolute: p.breakdown.investments,
          investmentsChange,
          investmentsPerf,
          pensionAbsolute: p.breakdown.pension,
          pensionChange,
          pensionPerf,
          personalAbsolute: p.breakdown.personal,
          personalChange,
          personalPerf,
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
              const data = payload[0].payload as {
                primaryAbsolute: number;
                primaryPerf: number;
                cashAbsolute: number;
                cashPerf: number;
                investmentsAbsolute: number;
                investmentsPerf: number;
                pensionAbsolute: number;
                pensionPerf: number;
                personalAbsolute: number;
                personalPerf: number;
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
                        {viewMode === "absolute"
                          ? formatCurrency(data.primaryAbsolute)
                          : formatPercent(data.primaryPerf)}
                      </span>
                    </div>
                    <div className="flex items-baseline justify-between gap-3">
                      <span className="text-[11px] uppercase text-muted-foreground">
                        Cash
                      </span>
                      <span className="font-semibold text-sm tabular-nums text-emerald-600">
                        {viewMode === "absolute"
                          ? formatCurrency(data.cashAbsolute)
                          : formatPercent(data.cashPerf)}
                      </span>
                    </div>
                    <div className="flex items-baseline justify-between gap-3">
                      <span className="text-[11px] uppercase text-muted-foreground">
                        Investments
                      </span>
                      <span className="font-semibold text-sm tabular-nums text-blue-600">
                        {viewMode === "absolute"
                          ? formatCurrency(data.investmentsAbsolute)
                          : formatPercent(data.investmentsPerf)}
                      </span>
                    </div>
                    <div className="flex items-baseline justify-between gap-3">
                      <span className="text-[11px] uppercase text-muted-foreground">
                        Pension
                      </span>
                      <span className="font-semibold text-sm tabular-nums text-purple-600">
                        {viewMode === "absolute"
                          ? formatCurrency(data.pensionAbsolute)
                          : formatPercent(data.pensionPerf)}
                      </span>
                    </div>
                    <div className="flex items-baseline justify-between gap-3">
                      <span className="text-[11px] uppercase text-muted-foreground">
                        Personal
                      </span>
                      <span className="font-semibold text-sm tabular-nums text-amber-600">
                        {viewMode === "absolute"
                          ? formatCurrency(data.personalAbsolute)
                          : formatPercent(data.personalPerf)}
                      </span>
                    </div>
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
          <Area
            type="monotone"
            dataKey={viewMode === "absolute" ? "cashChange" : "cashPerf"}
            stroke="#10b981"
            strokeWidth={2}
            fill="url(#cashGradient)"
            fillOpacity={1}
            isAnimationActive={false}
            name="Cash"
          />
          <Area
            type="monotone"
            dataKey={
              viewMode === "absolute" ? "investmentsChange" : "investmentsPerf"
            }
            stroke="#3b82f6"
            strokeWidth={2}
            fill="url(#investmentsGradient)"
            fillOpacity={1}
            isAnimationActive={false}
            name="Investments"
          />
          <Area
            type="monotone"
            dataKey={viewMode === "absolute" ? "pensionChange" : "pensionPerf"}
            stroke="#a855f7"
            strokeWidth={2}
            fill="url(#pensionGradient)"
            fillOpacity={1}
            isAnimationActive={false}
            name="Pension"
          />
          <Area
            type="monotone"
            dataKey={
              viewMode === "absolute" ? "personalChange" : "personalPerf"
            }
            stroke="#f59e0b"
            strokeWidth={2}
            fill="url(#personalGradient)"
            fillOpacity={1}
            isAnimationActive={false}
            name="Personal"
          />
          <Area
            type="monotone"
            dataKey={viewMode === "absolute" ? "primaryChange" : "primaryPerf"}
            stroke="#6366f1"
            strokeWidth={3}
            fill="url(#nwGradientNominal)"
            fillOpacity={1}
            isAnimationActive={false}
            name="Net Worth"
          />
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
