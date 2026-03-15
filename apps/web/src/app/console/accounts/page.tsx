import { getAccountBalances } from "@/lib/api";

export default async function AccountsPage() {
  const accounts = await getAccountBalances();

  return (
    <section className="rounded-xl border border-(--line) bg-(--surface) p-5">
      <h2 className="text-xl font-semibold">Accounts</h2>
      <p className="mt-2 text-sm text-(--ink-2)">
        Account inventory and latest balances by institution.
      </p>

      <ul className="mt-4 space-y-3">
        {accounts.map((account) => (
          <li
            key={account.accountId}
            className="rounded-lg border border-(--line) p-4"
          >
            <p className="text-sm text-(--ink-2)">{account.institution}</p>
            <p className="text-lg font-semibold">{account.accountName}</p>
            <p className="font-mono">
              {account.balance.toLocaleString(undefined, {
                style: "currency",
                currency: account.currency,
              })}
            </p>
          </li>
        ))}
      </ul>
    </section>
  );
}
