import Link from "next/link";

export default function Home() {
  return (
    <main className="matapan-shell matapan-section">
      <div className="flex flex-col gap-4 border-b border-[var(--line)] pb-6">
        <p className="text-xs tracking-[0.22em] text-[var(--ink-2)] uppercase">
          Matapan / UI Seed
        </p>
        <h1 className="text-3xl font-semibold leading-tight md:text-4xl">
          Net-worth dashboard and operator console
        </h1>
        <p className="max-w-2xl text-[var(--ink-2)]">
          First implementation milestone with route scaffolding and typed contract
          boundaries.
        </p>
      </div>

      <section className="matapan-grid mt-6 sm:grid-cols-2">
        <Link
          href="/dashboard"
          className="rounded-xl border border-[var(--line)] bg-[var(--surface)] p-5 transition hover:-translate-y-0.5 hover:shadow-md"
        >
          <p className="text-xs tracking-[0.16em] text-[var(--ink-2)] uppercase">
            Read / Insight
          </p>
          <h2 className="mt-2 text-xl font-semibold">Dashboard</h2>
          <p className="mt-2 text-sm text-[var(--ink-2)]">
            Net worth trend, currency splits, and recent parser runs.
          </p>
        </Link>

        <Link
          href="/console"
          className="rounded-xl border border-[var(--line)] bg-[var(--surface)] p-5 transition hover:-translate-y-0.5 hover:shadow-md"
        >
          <p className="text-xs tracking-[0.16em] text-[var(--ink-2)] uppercase">
            Write / Control
          </p>
          <h2 className="mt-2 text-xl font-semibold">Data Console</h2>
          <p className="mt-2 text-sm text-[var(--ink-2)]">
            Transactions, accounts, and rule management workflows.
          </p>
        </Link>
      </section>
    </main>
  );
}
