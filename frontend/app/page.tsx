"use client";

/**
 * Main Dashboard Page
 * Displays net worth overview with KPIs, charts, and snapshot history
 */

import { AssetsBreakdownChart } from "@/components/dashboard/assets-breakdown-chart";
import { NetWorthChart } from "@/components/dashboard/net-worth-chart";
import { SnapshotTable } from "@/components/dashboard/snapshot-table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { useDashboard } from "@/hooks/use-dashboard";
import type { SnapshotBreakdown } from "@/lib/types";
import { RefreshCw } from "lucide-react";

function formatCurrency(amount: number, currency: string = "EUR"): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency,
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  }).format(amount);
}

function formatDate(dateString: string): string {
  return new Date(dateString).toLocaleString("en-US", {
    year: "numeric",
    month: "long",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

import { useMemo, useState } from "react";

// Simple iPhone-style toggle switch (local, minimal styling)
interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: string;
}

function Switch({ checked, onChange, label }: SwitchProps) {
  return (
    <div className="flex items-center gap-2 select-none">
      {label && (
        <span className="text-xs font-medium text-muted-foreground">
          {label}
        </span>
      )}
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={
          "relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500 " +
          (checked ? "bg-indigo-600" : "bg-gray-300")
        }
      >
        <span
          className={
            "inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform " +
            (checked ? "translate-x-5" : "translate-x-1")
          }
        />
      </button>
    </div>
  );
}

export default function Home() {
  const [netWorthPercentChange, setNetWorthPercentChange] = useState<
    number | null
  >(null);
  const { data: dashboard, isLoading, error, refetch } = useDashboard(30000); // Poll every 30 seconds
  const [showInflation, setShowInflation] = useState(false);

  // Hooks must run regardless of loading state; handle undefined within memo.
  const processedSnapshots = useMemo(() => {
    if (!dashboard?.snapshots) return [];
    if (!showInflation) return dashboard.snapshots;
    return dashboard.snapshots.map((s) => {
      const scale = s.inflation_adjusted?.scale ?? 1.0;
      const scaledBreakdown: SnapshotBreakdown = {
        cash: s.breakdown.cash * scale,
        investments: s.breakdown.investments * scale,
        personal: s.breakdown.personal * scale,
        pension: s.breakdown.pension * scale,
        liabilities: s.breakdown.liabilities * scale,
      };
      const scaledTotals = {
        assets: s.totals.assets * scale,
        liabilities: s.totals.liabilities * scale,
        net_worth: s.totals.net_worth * scale,
      };
      return { ...s, breakdown: scaledBreakdown, totals: scaledTotals };
    });
  }, [dashboard, showInflation]);

  const processedLatest = useMemo(() => {
    const l = dashboard?.latest;
    if (!l) return null;
    if (!showInflation) return l;
    const scale = l.inflation_adjusted?.scale ?? 1.0;
    const scaledBreakdown: SnapshotBreakdown = {
      cash: l.breakdown.cash * scale,
      investments: l.breakdown.investments * scale,
      personal: l.breakdown.personal * scale,
      pension: l.breakdown.pension * scale,
      liabilities: l.breakdown.liabilities * scale,
    };
    const scaledTotals = {
      assets: l.totals.assets * scale,
      liabilities: l.totals.liabilities * scale,
      net_worth: l.totals.net_worth * scale,
    };
    return { ...l, breakdown: scaledBreakdown, totals: scaledTotals };
  }, [dashboard, showInflation]);

  if (error) {
    return (
      <div className="container mx-auto p-8">
        <Card className="border-destructive">
          <CardHeader>
            <CardTitle className="text-destructive">
              Error Loading Dashboard
            </CardTitle>
            <CardDescription>Failed to fetch dashboard data</CardDescription>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground mb-4">
              {error.message}
            </p>
            <Button onClick={() => refetch()} variant="outline">
              <RefreshCw className="mr-2 h-4 w-4" />
              Retry
            </Button>
          </CardContent>
        </Card>
      </div>
    );
  }

  if (isLoading || !dashboard || !processedLatest) {
    return (
      <div className="container mx-auto p-8">
        <div className="mb-8">
          <Skeleton className="h-8 w-64 mb-2" />
          <Skeleton className="h-4 w-48" />
        </div>
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4 mb-8">
          {[...Array(4)].map((_, i) => (
            <Card key={i}>
              <CardHeader>
                <Skeleton className="h-4 w-24" />
              </CardHeader>
              <CardContent>
                <Skeleton className="h-8 w-32" />
              </CardContent>
            </Card>
          ))}
        </div>
      </div>
    );
  }

  const { metadata } = dashboard;
  const latestOriginal = dashboard.latest; // original (unscaled) for purchasing power reference

  return (
    <div className="container mx-auto p-8">
      {/* Header */}
      <div className="flex flex-col md:flex-row md:justify-between md:items-start mb-8">
        <div className="mb-4 md:mb-0">
          <h1 className="text-4xl font-bold tracking-tight mb-2">
            Net Worth Dashboard
          </h1>
          <p className="text-muted-foreground">
            Last updated: {formatDate(metadata.generated_at)}
          </p>
          <p className="text-xs text-muted-foreground mt-1">
            View:{" "}
            {showInflation
              ? "Inflation-adjusted (HICP deflated)"
              : "Nominal values"}
          </p>
        </div>
        <div className="flex flex-col items-end gap-3">
          <Button
            onClick={() => refetch()}
            variant="outline"
            size="sm"
            className="self-end"
          >
            <RefreshCw className="mr-2 h-4 w-4" />
            Refresh
          </Button>
          <div className="flex flex-col items-end gap-1">
            <Switch checked={showInflation} onChange={setShowInflation} />
            <span className="text-xs text-muted-foreground">Inflation</span>
          </div>
        </div>
      </div>

      {/* Key Metrics */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4 mb-8">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Net Worth
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold">
              {formatCurrency(processedLatest.totals.net_worth)}
            </div>
            <p className="text-xs text-muted-foreground mt-1">
              As of {processedLatest.reference_month}
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Total Assets
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold text-green-600">
              {formatCurrency(processedLatest.totals.assets)}
            </div>
            <p className="text-xs text-muted-foreground mt-1">
              Cash, Investments, Pension
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Liabilities
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold text-red-600">
              {formatCurrency(processedLatest.totals.liabilities)}
            </div>
            <p className="text-xs text-muted-foreground mt-1">Loans & Debts</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Purchasing Power
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              +
              {latestOriginal.real_purchasing_power.ny_advantage_pct.toFixed(1)}
              %
            </div>
            <Badge variant="secondary" className="mt-2 text-xs">
              vs New York
            </Badge>
          </CardContent>
        </Card>
      </div>

      {/* Charts */}
      <div className="grid gap-6 md:grid-cols-2 mb-8">
        <Card>
          <CardHeader>
            <CardTitle>Assets Breakdown</CardTitle>
            <CardDescription>Current distribution of assets</CardDescription>
          </CardHeader>
          <CardContent>
            <AssetsBreakdownChart breakdown={processedLatest.breakdown} />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <div className="flex items-center gap-2">
              <CardTitle className="flex items-center gap-2">
                Net Worth Over Time
                {netWorthPercentChange !== null && (
                  <Badge
                    variant="secondary"
                    className={
                      netWorthPercentChange >= 0
                        ? "text-green-600 text-xs font-semibold"
                        : "text-red-600 text-xs font-semibold"
                    }
                  >
                    {netWorthPercentChange >= 0 ? "+" : ""}
                    {netWorthPercentChange.toFixed(1)}%
                  </Badge>
                )}
              </CardTitle>
            </div>
            <CardDescription>
              Historical trend of your net worth
            </CardDescription>
          </CardHeader>
          <CardContent>
            <NetWorthChart
              snapshots={
                showInflation ? processedSnapshots : processedSnapshots
              }
              comparisonSnapshots={
                showInflation ? dashboard.snapshots : undefined
              }
              onPercentChange={(pct) => setNetWorthPercentChange(pct)}
            />
          </CardContent>
        </Card>
      </div>

      {/* Snapshot Table */}
      <Card>
        <CardHeader>
          <CardTitle>Snapshot History</CardTitle>
          <CardDescription>
            Monthly snapshots showing{" "}
            {showInflation ? "inflation-adjusted" : "nominal"} values
          </CardDescription>
        </CardHeader>
        <CardContent>
          <SnapshotTable snapshots={processedSnapshots} />
        </CardContent>
      </Card>
    </div>
  );
}
