import { getAccountBalances, getNetWorthSeries, getParserRuns } from "@/lib/api";

export default async function DashboardPage() {
  const [netWorth, accounts, runs] = await Promise.all([
    getNetWorthSeries(),
    getAccountBalances(),
    getParserRuns(),
  ]);

  const latestPoint = netWorth.points.at(-1);

  return (
    <main className="matapan-shell matapan-section">
      <header className="border-b border-(--line) pb-5">
        <p className="text-xs tracking-[0.2em] text-(--ink-2) uppercase">
          Dashboard
        </p>
        <h1 className="mt-2 text-3xl font-semibold">Net worth overview</h1>
        <p className="mt-2 text-sm text-(--ink-2)">
          Base currency: {netWorth.baseCurrency}
        </p>
      </header>

      <section className="matapan-grid mt-6 md:grid-cols-3">
        <article className="rounded-xl border border-(--line) bg-(--surface) p-4">
          <p className="text-xs text-(--ink-2) uppercase tracking-[0.14em]">
            Current net worth
          </p>
          <p className="mt-2 font-mono text-2xl">
            {latestPoint?.amountBaseCcy.toLocaleString(undefined, {
              style: "currency",
              currency: netWorth.baseCurrency,
            })}
          </p>
          <p className="mt-1 text-xs text-(--ink-2)">as of {latestPoint?.date}</p>
        </article>

        <article className="rounded-xl border border-(--line) bg-(--surface) p-4">
          <p className="text-xs text-(--ink-2) uppercase tracking-[0.14em]">
            Accounts tracked
          </p>
          <p className="mt-2 font-mono text-2xl">{accounts.length}</p>
          <p className="mt-1 text-xs text-(--ink-2)">multi-institution sample data</p>
        </article>

        <article className="rounded-xl border border-(--line) bg-(--surface) p-4">
          <p className="text-xs text-(--ink-2) uppercase tracking-[0.14em]">
            Latest parser run
          </p>
          <p className="mt-2 font-mono text-base">{runs[0]?.source}</p>
          <p className="mt-1 text-xs text-(--ink-2)">status: {runs[0]?.status}</p>
        </article>
      </section>

      <section className="matapan-grid mt-6 md:grid-cols-2">
        <article className="rounded-xl border border-(--line) bg-(--surface) p-4">
          <h2 className="text-lg font-semibold">Trend points</h2>
          <ul className="mt-3 space-y-2 text-sm">
            {netWorth.points.map((point) => (
              <li
                key={point.date}
                className="flex items-center justify-between border-b border-(--line) pb-2"
              >
                <span>{point.date}</span>
                <span className="font-mono">
                  {point.amountBaseCcy.toLocaleString(undefined, {
                    style: "currency",
                    currency: netWorth.baseCurrency,
                  })}
                </span>
              </li>
            ))}
          </ul>
        </article>

        <article className="rounded-xl border border-(--line) bg-(--surface) p-4">
          <h2 className="text-lg font-semibold">Recent parser runs</h2>
          <ul className="mt-3 space-y-2 text-sm">
            {runs.map((run) => (
              <li key={run.runId} className="rounded-lg border border-(--line) p-3">
                <p className="font-mono text-xs">{run.runId}</p>
                <p className="mt-1">{run.source}</p>
                <p className="text-xs text-(--ink-2)">
                  {run.status} / rows imported: {run.importedRows}
                </p>
              </li>
            ))}
          </ul>
        </article>
      </section>
    </main>
  );
}
