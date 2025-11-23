"use client";

/**
 * Main Dashboard Page
 * Displays net worth overview with KPIs, charts, and snapshot history
 */

import { NetWorthChart } from "@/components/dashboard/net-worth-chart";
import { SnapshotTable } from "@/components/dashboard/snapshot-table";
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
import { RefreshCw } from "lucide-react";

function formatCurrency(amount: number): string {
  // Plain numeric formatting (remove currency symbol; base currency displayed in header)
  return new Intl.NumberFormat("en-US", {
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

// Removed inflation toggle; backend provides only nominal + real wealth metrics

export default function Home() {
  const [netWorthPercentChange, setNetWorthPercentChange] = useState<
    number | null
  >(null);
  const { data: dashboard, isLoading, error, refetch } = useDashboard(30000); // Poll every 30 seconds
  // Percent change computed directly; no inflation-adjusted toggle

  // Hooks must run regardless of loading state; handle undefined within memo.
  const processedSnapshots = useMemo(
    () => dashboard?.snapshots || [],
    [dashboard]
  );

  const processedLatest = useMemo(
    () => dashboard?.snapshots?.[dashboard.snapshots.length - 1] || null,
    [dashboard]
  );

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
          <p className="text-xs text-muted-foreground mt-1 flex flex-wrap gap-4">
            <span>Base Currency: {metadata.base_currency}</span>
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

        {/* Removed Purchasing Power card (not provided by backend) */}

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Net Cash Flow
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold">
              {formatCurrency(processedLatest.totals.net_cash_flow)}
            </div>
            <p className="text-xs text-muted-foreground mt-1">
              Monthly income minus expenses
            </p>
          </CardContent>
        </Card>
      </div>

      {/* Charts */}
      <Card className="mb-8">
        <CardHeader>
          <div className="flex items-center gap-2">
            <CardTitle className="flex items-center gap-2">
              Net Worth Over Time
              {netWorthPercentChange !== null && (
                <span
                  className={
                    (netWorthPercentChange >= 0
                      ? "text-green-600"
                      : "text-red-600") + " text-xs font-semibold ml-2"
                  }
                >
                  {netWorthPercentChange >= 0 ? "+" : ""}
                  {netWorthPercentChange.toFixed(1)}%
                </span>
              )}
            </CardTitle>
          </div>
          <CardDescription>
            Historical trend of your net worth and asset breakdown
          </CardDescription>
        </CardHeader>
        <CardContent>
          <NetWorthChart
            snapshots={processedSnapshots}
            onPercentChange={(pct) => setNetWorthPercentChange(pct)}
          />
        </CardContent>
      </Card>

      {/* Snapshot Table */}
      <Card>
        <CardHeader>
          <CardTitle>Snapshot History</CardTitle>
          <CardDescription>Monthly snapshots (nominal values)</CardDescription>
        </CardHeader>
        <CardContent>
          <SnapshotTable snapshots={processedSnapshots} />
        </CardContent>
      </Card>
    </div>
  );
}
