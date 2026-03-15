import { getAccountBalances, getNetWorthSeries, getTransactions } from "@/lib/api";

export default async function DashboardPage() {
  const [netWorth, accounts, transactions] = await Promise.all([
    getNetWorthSeries(),
    getAccountBalances(),
    getTransactions(),
  ]);

  const latestPoint = netWorth.points.at(-1);
  const noData =
    netWorth.points.length === 0 && accounts.length === 0 && transactions.length === 0;
  const recentTransactions = transactions.slice(0, 8);

  if (noData) {
    return (
      <main className="matapan-shell matapan-section">
        <header className="border-b border-(--line) pb-5">
          <p className="text-xs tracking-[0.2em] text-(--ink-2) uppercase">
            Dashboard
          </p>
          <h1 className="mt-2 text-3xl font-semibold">Net worth overview</h1>
        </header>

        <section className="mt-6 rounded-xl border border-(--line) bg-(--surface) p-5">
          <h2 className="text-lg font-semibold">No data available yet</h2>
          <p className="mt-2 text-sm text-(--ink-2)">
            The database file is missing or empty. Add data to database/database.json
            and refresh this page.
          </p>
        </section>
      </main>
    );
  }

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
          {latestPoint ? (
            <>
              <p className="mt-2 font-mono text-2xl">
                {latestPoint.amountBaseCcy.toLocaleString(undefined, {
                  style: "currency",
                  currency: netWorth.baseCurrency,
                })}
              </p>
              <p className="mt-1 text-xs text-(--ink-2)">as of {latestPoint.date}</p>
            </>
          ) : (
            <p className="mt-2 text-sm text-(--ink-2)">No net-worth snapshots yet</p>
          )}
        </article>

        <article className="rounded-xl border border-(--line) bg-(--surface) p-4">
          <p className="text-xs text-(--ink-2) uppercase tracking-[0.14em]">
            Accounts tracked
          </p>
          <p className="mt-2 font-mono text-2xl">{accounts.length}</p>
          <p className="mt-1 text-xs text-(--ink-2)">loaded from database.json</p>
        </article>

        <article className="rounded-xl border border-(--line) bg-(--surface) p-4">
          <p className="text-xs text-(--ink-2) uppercase tracking-[0.14em]">
            Transactions loaded
          </p>
          <p className="mt-2 font-mono text-2xl">{transactions.length}</p>
          <p className="mt-1 text-xs text-(--ink-2)">all currencies</p>
        </article>
      </section>

      <section className="matapan-grid mt-6 md:grid-cols-2">
        <article className="rounded-xl border border-(--line) bg-(--surface) p-4">
          <h2 className="text-lg font-semibold">Trend points</h2>
          {netWorth.points.length === 0 ? (
            <p className="mt-3 text-sm text-(--ink-2)">No trend points yet</p>
          ) : (
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
          )}
        </article>

        <article className="rounded-xl border border-(--line) bg-(--surface) p-4">
          <h2 className="text-lg font-semibold">Recent transactions</h2>
          {recentTransactions.length === 0 ? (
            <p className="mt-3 text-sm text-(--ink-2)">No transactions yet</p>
          ) : (
            <ul className="mt-3 space-y-2 text-sm">
              {recentTransactions.map((tx) => (
                <li key={tx.id} className="rounded-lg border border-(--line) p-3">
                  <p className="font-mono text-xs">{tx.bookingDate}</p>
                  <p className="mt-1">{tx.description}</p>
                  <p className="text-xs text-(--ink-2)">
                    {tx.amount.toLocaleString(undefined, {
                      style: "currency",
                      currency: tx.currency,
                    })}
                  </p>
                </li>
              ))}
            </ul>
          )}
        </article>
      </section>
    </main>
  );
}
