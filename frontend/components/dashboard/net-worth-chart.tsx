"use client";

/**
 * Net Worth Chart Component
 * Displays a line/area chart showing net worth over time
 */

import type { Snapshot } from "@/lib/types";
import {
  Area,
  AreaChart,
  CartesianGrid,
  Legend,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

interface NetWorthChartProps {
  snapshots: Snapshot[];
}

export function NetWorthChart({ snapshots }: NetWorthChartProps) {
  const data = snapshots.map((snapshot) => ({
    month: snapshot.reference_month,
    "Net Worth": snapshot.totals.net_worth,
    Assets: snapshot.totals.assets,
    Liabilities: snapshot.totals.liabilities,
  }));

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

  return (
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
          formatter={formatCurrency}
          labelFormatter={(label) => `Month: ${label}`}
        />
        <Legend />
        <Area
          type="monotone"
          dataKey="Assets"
          stackId="1"
          stroke="#10b981"
          fill="#10b981"
          fillOpacity={0.6}
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
  );
}
