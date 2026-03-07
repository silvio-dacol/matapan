import Link from "next/link";

const consoleLinks = [
  { href: "/console/transactions", label: "Transactions" },
  { href: "/console/accounts", label: "Accounts" },
  { href: "/console/rules", label: "Rules" },
];

export default function ConsoleLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <main className="matapan-shell matapan-section">
      <header className="border-b border-[var(--line)] pb-5">
        <p className="text-xs tracking-[0.2em] text-[var(--ink-2)] uppercase">
          Data Console
        </p>
        <h1 className="mt-2 text-3xl font-semibold">Operator controls</h1>
        <p className="mt-2 text-sm text-[var(--ink-2)]">
          Safe write workflows with explicit visibility and review steps.
        </p>
      </header>

      <nav className="mt-5 flex flex-wrap gap-2">
        {consoleLinks.map((link) => (
          <Link
            key={link.href}
            href={link.href}
            className="rounded-full border border-[var(--line)] bg-[var(--surface)] px-4 py-2 text-sm transition hover:bg-[var(--brand-soft)]"
          >
            {link.label}
          </Link>
        ))}
      </nav>

      <section className="mt-6">{children}</section>
    </main>
  );
}
