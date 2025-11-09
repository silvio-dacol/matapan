"use client";

/**
 * Assets Breakdown Chart Component
 * Displays a pie/donut chart showing the distribution of assets
 */

import type { SnapshotBreakdown } from "@/lib/types";
import {
  Cell,
  Legend,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
} from "recharts";

interface AssetsBreakdownChartProps {
  breakdown: SnapshotBreakdown;
}

const COLORS = {
  cash: "#10b981", // green
  investments: "#3b82f6", // blue
  pension: "#8b5cf6", // purple
  personal: "#f59e0b", // amber
};

export function AssetsBreakdownChart({ breakdown }: AssetsBreakdownChartProps) {
  const data = [
    { name: "Cash", value: breakdown.cash, color: COLORS.cash },
    {
      name: "Investments",
      value: breakdown.investments,
      color: COLORS.investments,
    },
    { name: "Pension", value: breakdown.pension, color: COLORS.pension },
    ...(breakdown.personal > 0
      ? [
          {
            name: "Personal",
            value: breakdown.personal,
            color: COLORS.personal,
          },
        ]
      : []),
  ].filter((item) => item.value > 0);

  const formatCurrency = (value: number) => {
    return new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: "EUR",
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    }).format(value);
  };

  return (
    <ResponsiveContainer width="100%" height={300}>
      <PieChart>
        <Pie
          data={data}
          cx="50%"
          cy="50%"
          labelLine={false}
          outerRadius={80}
          fill="#8884d8"
          dataKey="value"
          label
        >
          {data.map((entry, index) => (
            <Cell key={`cell-${index}`} fill={entry.color} />
          ))}
        </Pie>
        <Tooltip formatter={formatCurrency} />
        <Legend />
      </PieChart>
    </ResponsiveContainer>
  );
}
