import { getTransactions } from "@/lib/api";

export default async function TransactionsPage() {
  const transactions = await getTransactions();

  return (
    <section className="rounded-xl border border-[var(--line)] bg-[var(--surface)] p-5">
      <h2 className="text-xl font-semibold">Transactions</h2>
      <p className="mt-2 text-sm text-[var(--ink-2)]">
        This table is backed by validated contracts and ready for server data.
      </p>

      <div className="mt-4 overflow-x-auto">
        <table className="min-w-full text-sm">
          <thead>
            <tr className="border-b border-[var(--line)] text-left">
              <th className="px-2 py-2">Date</th>
              <th className="px-2 py-2">Account</th>
              <th className="px-2 py-2">Description</th>
              <th className="px-2 py-2">Category</th>
              <th className="px-2 py-2">Amount</th>
              <th className="px-2 py-2">Enriched</th>
            </tr>
          </thead>
          <tbody>
            {transactions.map((tx) => (
              <tr key={tx.id} className="border-b border-[var(--line)]">
                <td className="px-2 py-2">{tx.bookingDate}</td>
                <td className="px-2 py-2">{tx.accountName}</td>
                <td className="px-2 py-2">{tx.description}</td>
                <td className="px-2 py-2">{tx.category ?? "Unassigned"}</td>
                <td className="px-2 py-2 font-mono">
                  {tx.amount.toLocaleString(undefined, {
                    style: "currency",
                    currency: tx.currency,
                  })}
                </td>
                <td className="px-2 py-2">{tx.enriched ? "Yes" : "No"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
