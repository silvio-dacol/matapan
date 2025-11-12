"use client";

/**
 * Assets Breakdown Chart Component
 * Displays a pie/donut chart showing the distribution of assets
 */

import type { SnapshotBreakdown } from "@/lib/types";
import { useMemo } from "react";
import { Cell, Pie, PieChart, ResponsiveContainer, Tooltip } from "recharts";

interface AssetsBreakdownChartProps {
  breakdown: SnapshotBreakdown;
}

// Harmonized palette aligned with other dashboard components
// Cash (emerald), Investments (indigo), Pension (violet), Personal (amber), plus extra slots for future categories
const COLORS = {
  cash: "#10b981", // emerald
  investments: "#6366f1", // indigo (matches net worth nominal line)
  pension: "#8b5cf6", // violet
  personal: "#f59e0b", // amber
  realEstate: "#dc2626", // red (reserved / future)
  crypto: "#0ea5e9", // cyan (reserved / future)
};

export function AssetsBreakdownChart({ breakdown }: AssetsBreakdownChartProps) {
  const data = useMemo(
    () =>
      [
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
      ].filter((item) => item.value > 0),
    [
      breakdown.cash,
      breakdown.investments,
      breakdown.pension,
      breakdown.personal,
    ]
  );

  const formatCurrency = (value: number) => {
    // Plain number formatting; currency symbol removed (shown globally elsewhere)
    return new Intl.NumberFormat("en-US", {
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    }).format(value);
  };

  return (
    <div className="space-y-3" aria-label="Assets breakdown chart">
      <ResponsiveContainer width="100%" height={260}>
        <PieChart margin={{ top: 8, bottom: 8, left: 0, right: 0 }}>
          <defs>
            {/* subtle shadow / gradient could be added here if desired */}
          </defs>
          <Pie
            data={data}
            cx="50%"
            cy="50%"
            innerRadius={58}
            outerRadius={100}
            paddingAngle={1}
            dataKey="value"
            nameKey="name"
            isAnimationActive={false}
          >
            {data.map((entry, index) => (
              <Cell
                key={`cell-${index}`}
                fill={entry.color}
                aria-label={`${entry.name} slice`}
              />
            ))}
          </Pie>
          <Tooltip
            cursor={{ fill: "hsl(var(--muted))", opacity: 0.15 }}
            content={({ active, payload }) => {
              if (!active || !payload || payload.length === 0) return null;
              const item = payload[0].payload as {
                name: string;
                value: number;
              };
              return (
                <div className="rounded-md border bg-background/95 backdrop-blur px-3 py-2 shadow-lg min-w-[140px] space-y-1">
                  <div className="text-xs font-medium tracking-wide text-muted-foreground">
                    {item.name}
                  </div>
                  <div className="flex items-baseline justify-between gap-2">
                    <span className="text-[11px] uppercase text-muted-foreground">
                      Value
                    </span>
                    <span className="font-semibold text-sm tabular-nums text-indigo-600">
                      {formatCurrency(item.value)}
                    </span>
                  </div>
                </div>
              );
            }}
          />
        </PieChart>
      </ResponsiveContainer>
      <div
        className="flex flex-wrap gap-2 justify-center pb-1"
        role="list"
        aria-label="Assets legend"
      >
        {data.map((d) => (
          <button
            key={d.name}
            type="button"
            className="flex items-center gap-1.5 text-xs sm:text-[13px] font-medium text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded px-1.5 py-0.5"
            aria-label={d.name}
            disabled
          >
            <span
              className="inline-block h-2.5 w-2.5 rounded-sm shadow"
              style={{ backgroundColor: d.color }}
              aria-hidden="true"
            />
            <span className="whitespace-nowrap">{d.name}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
