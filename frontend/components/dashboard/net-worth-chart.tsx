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

  const formatCurrency = (value: number) =>
    new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: "EUR",
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
            contentStyle={{ borderRadius: "4px" }}
            formatter={(_, __, item) => {
              const pAbs = item.payload.primaryAbsolute;
              const cAbs = item.payload.comparisonAbsolute;
              if (cAbs !== undefined) {
                return [
                  `${formatCurrency(
                    pAbs
                  )} (Inflation-adjusted)\n${formatCurrency(cAbs)} (Nominal)`,
                  "Net Worth",
                ];
              }
              return [formatCurrency(pAbs), "Net Worth"];
            }}
            labelFormatter={(label) => label}
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
