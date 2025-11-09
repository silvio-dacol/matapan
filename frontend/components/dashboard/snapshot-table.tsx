"use client";

/**
 * Snapshot Table Component
 * Displays a table of all monthly snapshots with key metrics
 */

import { Badge } from "@/components/ui/badge";
import type { Snapshot } from "@/lib/types";

interface SnapshotTableProps {
  snapshots: Snapshot[];
}

function formatCurrency(amount: number, currency: string = "EUR"): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency,
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  }).format(amount);
}

export function SnapshotTable({ snapshots }: SnapshotTableProps) {
  // Reverse to show most recent first
  const sortedSnapshots = [...snapshots].reverse();

  return (
    <div className="overflow-x-auto">
      <table className="w-full">
        <thead>
          <tr className="border-b">
            <th className="text-left py-3 px-4 font-medium">Month</th>
            <th className="text-right py-3 px-4 font-medium">Net Worth</th>
            <th className="text-right py-3 px-4 font-medium">Assets</th>
            <th className="text-right py-3 px-4 font-medium">Cash</th>
            <th className="text-right py-3 px-4 font-medium">Investments</th>
            <th className="text-right py-3 px-4 font-medium">Pension</th>
            <th className="text-right py-3 px-4 font-medium">Liabilities</th>
            <th className="text-right py-3 px-4 font-medium">PP vs NY</th>
          </tr>
        </thead>
        <tbody>
          {sortedSnapshots.map((snapshot, index) => (
            <tr
              key={snapshot.reference_month}
              className={`border-b hover:bg-muted/50 ${
                index === 0 ? "font-semibold" : ""
              }`}
            >
              <td className="py-3 px-4">
                {snapshot.reference_month}
                {index === 0 && (
                  <Badge variant="secondary" className="ml-2 text-xs">
                    Latest
                  </Badge>
                )}
              </td>
              <td className="text-right py-3 px-4 font-medium">
                {formatCurrency(snapshot.totals.net_worth)}
              </td>
              <td className="text-right py-3 px-4 text-green-600">
                {formatCurrency(snapshot.totals.assets)}
              </td>
              <td className="text-right py-3 px-4 text-muted-foreground">
                {formatCurrency(snapshot.breakdown.cash)}
              </td>
              <td className="text-right py-3 px-4 text-muted-foreground">
                {formatCurrency(snapshot.breakdown.investments)}
              </td>
              <td className="text-right py-3 px-4 text-muted-foreground">
                {formatCurrency(snapshot.breakdown.pension)}
              </td>
              <td className="text-right py-3 px-4 text-red-600">
                {formatCurrency(snapshot.totals.liabilities)}
              </td>
              <td className="text-right py-3 px-4">
                <span className="text-sm text-muted-foreground">
                  +{snapshot.real_purchasing_power.ny_advantage_pct.toFixed(1)}%
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
