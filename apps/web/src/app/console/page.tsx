import Link from "next/link";

export default function ConsoleIndexPage() {
  return (
    <section className="rounded-xl border border-(--line) bg-(--surface) p-5">
      <h2 className="text-xl font-semibold">Choose an area</h2>
      <p className="mt-2 text-sm text-(--ink-2)">
        Start from transaction review, account visibility, or rule management.
      </p>
      <div className="mt-4 flex flex-wrap gap-2 text-sm">
        <Link href="/console/transactions" className="underline">
          Open transactions
        </Link>
        <Link href="/console/accounts" className="underline">
          Open accounts
        </Link>
        <Link href="/console/rules" className="underline">
          Open rules
        </Link>
      </div>
    </section>
  );
}
