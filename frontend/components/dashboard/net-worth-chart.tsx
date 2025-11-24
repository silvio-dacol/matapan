"use client";

import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { Snapshot } from "@/lib/types";
import { useMemo, useState } from "react";
import {
  CartesianGrid,
  Line,
  LineChart,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

interface NetWorthChartProps {
  snapshots: Snapshot[];
}

type ViewMode = "value" | "performance";
type RangeKey = "3M" | "YTD" | "1Y" | "3Y" | "5Y" | "ALL";

const RANGES: RangeKey[] = ["3M", "YTD", "1Y", "3Y", "5Y", "ALL"];

type Point = {
  index: number;
  label: string;
  cumulativeRealPerfPct: number;
  monthlyRealPerfPct: number;
  twrFactor: number;
  netWorth: number;
};

function buildPerformanceSeries(sortedSnapshots: Snapshot[]): Point[] {
  const ordered = [...sortedSnapshots].sort((a, b) =>
    a.reference_month.localeCompare(b.reference_month)
  );

  const points: Point[] = [];
  let cumulativeRealFactor = 1;

  ordered.forEach((snap, idx) => {
    const monthlyReal = snap.performance?.portfolio_real_return ?? 0;

    if (idx === 0) {
      cumulativeRealFactor = 1;
    } else {
      cumulativeRealFactor *= 1 + monthlyReal;
    }

    points.push({
      index: idx,
      label: snap.reference_month, // "YYYY-MM"
      cumulativeRealPerfPct: (cumulativeRealFactor - 1) * 100,
      monthlyRealPerfPct: monthlyReal * 100,
      twrFactor: snap.performance?.twr_cumulative ?? cumulativeRealFactor,
      netWorth: snap.totals.net_worth,
    });
  });

  return points;
}

function filterByRange(points: Point[], range: RangeKey): Point[] {
  if (points.length === 0) return points;
  const n = points.length;

  if (range === "ALL") return points;

  if (range === "YTD") {
    const latestYear = points[n - 1].label.slice(0, 4);
    const firstIdx = points.findIndex((p) => p.label.startsWith(latestYear));
    return points.slice(firstIdx >= 0 ? firstIdx : 0);
  }

  const monthsBackMap: Record<Exclude<RangeKey, "YTD" | "ALL">, number> = {
    "3M": 3,
    "1Y": 12,
    "3Y": 36,
    "5Y": 60,
  };

  const monthsBack = monthsBackMap[range as Exclude<RangeKey, "YTD" | "ALL">];
  const start = Math.max(n - monthsBack, 0);
  return points.slice(start);
}

function formatPct(value: number) {
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`;
}

function formatNumber(value: number) {
  return new Intl.NumberFormat("en-US", {
    maximumFractionDigits: 0,
  }).format(value);
}

interface CustomTooltipProps {
  mode: ViewMode;
  active?: boolean;
  payload?: any[];
  label?: string;
}

const CustomTooltip = ({
  mode,
  active,
  payload,
  label,
}: CustomTooltipProps) => {
  if (!active || !payload || payload.length === 0) return null;
  const data = payload[0].payload as Point;

  return (
    <div className="rounded-xl border bg-background/95 p-3 shadow-md">
      <div className="mb-1 text-xs text-muted-foreground">{label}</div>

      {mode === "value" ? (
        <>
          <div className="text-sm font-semibold">
            Net worth: {formatNumber(data.netWorth)}
          </div>
          <div className="text-xs text-muted-foreground">
            Real performance (cumulative):{" "}
            {formatPct(data.cumulativeRealPerfPct)}
          </div>
          <div className="text-xs text-muted-foreground">
            Monthly real performance: {formatPct(data.monthlyRealPerfPct)}
          </div>
        </>
      ) : (
        <>
          <div className="text-sm font-semibold">
            Real performance (cumulative):{" "}
            {formatPct(data.cumulativeRealPerfPct)}
          </div>
          <div className="text-xs text-muted-foreground">
            Monthly real performance: {formatPct(data.monthlyRealPerfPct)}
          </div>
          <div className="text-xs text-muted-foreground">
            TWR factor: {data.twrFactor.toFixed(4)}
          </div>
          <div className="mt-1 text-xs text-muted-foreground">
            Net worth: {formatNumber(data.netWorth)}
          </div>
        </>
      )}
    </div>
  );
};

export function NetWorthChart({ snapshots }: NetWorthChartProps) {
  const [mode, setMode] = useState<ViewMode>("value");
  const [range, setRange] = useState<RangeKey>("YTD");

  if (!snapshots || snapshots.length === 0) {
    return <div className="text-sm text-muted-foreground">No data yet</div>;
  }

  const fullSeries = useMemo(
    () => buildPerformanceSeries(snapshots),
    [snapshots]
  );
  const series = useMemo(
    () => filterByRange(fullSeries, range),
    [fullSeries, range]
  );

  // Y axis domain based on current mode
  let domain: [number, number];
  let tickFormatter: (value: number) => string;

  if (mode === "performance") {
    const values = series.map((p) => p.cumulativeRealPerfPct);
    const minVal = Math.min(...values, 0);
    const maxVal = Math.max(...values, 0);
    const padding = Math.max((maxVal - minVal) * 0.1, 5);
    domain = [minVal - padding, maxVal + padding];
    tickFormatter = (v) => `${v.toFixed(0)}%`;
  } else {
    const values = series.map((p) => p.netWorth);
    const minVal = Math.min(...values);
    const maxVal = Math.max(...values);
    const padding = Math.max((maxVal - minVal) * 0.05, maxVal * 0.01, 500);
    domain = [minVal - padding, maxVal + padding];
    tickFormatter = (v) => formatNumber(v);
  }

  return (
    <div className="w-full space-y-4">
      <div className="flex items-center justify-between">
        <Tabs value={mode} onValueChange={(v) => setMode(v as ViewMode)}>
          <TabsList className="grid grid-cols-2">
            <TabsTrigger value="value">Value</TabsTrigger>
            <TabsTrigger value="performance">Performance</TabsTrigger>
          </TabsList>
        </Tabs>

        <div className="inline-flex items-center rounded-full border bg-background p-1 text-xs">
          {RANGES.map((r) => (
            <button
              key={r}
              type="button"
              onClick={() => setRange(r)}
              className={[
                "rounded-full px-2 py-1",
                range === r
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-muted",
              ].join(" ")}
            >
              {r}
            </button>
          ))}
        </div>
      </div>

      <div className="h-80 w-full">
        <ResponsiveContainer width="100%" height="100%">
          <LineChart
            data={series}
            margin={{ top: 20, right: 30, bottom: 20, left: 0 }}
          >
            <CartesianGrid vertical={false} strokeOpacity={0.25} />
            <XAxis
              dataKey="label"
              tick={{ fontSize: 10 }}
              tickMargin={8}
              minTickGap={16}
            />
            <YAxis
              tickFormatter={tickFormatter}
              domain={domain}
              tick={{ fontSize: 12 }}
              width={70}
            />
            {mode === "performance" && (
              <ReferenceLine y={0} strokeOpacity={0.6} />
            )}
            <Tooltip content={<CustomTooltip mode={mode} />} />
            <Line
              type="monotone"
              dataKey={
                mode === "performance" ? "cumulativeRealPerfPct" : "netWorth"
              }
              strokeWidth={2}
              dot={false}
              activeDot={{ r: 4 }}
              name={mode === "performance" ? "Real performance" : "Net worth"}
            />
          </LineChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
