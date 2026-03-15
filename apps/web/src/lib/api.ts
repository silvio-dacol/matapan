import { readFile } from "node:fs/promises";
import path from "node:path";

import {
  parseDatabaseSnapshot,
  type AccountBalance,
  type DatabaseSnapshot,
  type NetWorthSeries,
  type ParserRun,
  type Transaction,
} from "@/lib/contracts";

function resolveDatabasePath(): string {
  const configuredPath = process.env.MATAPAN_DATABASE_PATH?.trim();
  if (configuredPath) {
    return path.isAbsolute(configuredPath)
      ? configuredPath
      : path.resolve(process.cwd(), configuredPath);
  }

  return path.resolve(process.cwd(), "..", "..", "database", "database.json");
}

async function loadDatabaseSnapshot(): Promise<DatabaseSnapshot | null> {
  const databasePath = resolveDatabasePath();

  try {
    const raw = await readFile(databasePath, "utf8");
    if (!raw.trim()) {
      return null;
    }

    return parseDatabaseSnapshot(JSON.parse(raw));
  } catch (error) {
    if (
      error &&
      typeof error === "object" &&
      "code" in error &&
      (error as { code?: string }).code === "ENOENT"
    ) {
      return null;
    }

    throw error;
  }
}

function prettyAccountName(accountId: string): string {
  return accountId
    .toLowerCase()
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function isPortfolioAccount(account: DatabaseSnapshot["accounts"][number]): boolean {
  return account.institution.toLowerCase() !== "external" && !account.account_id.startsWith("EXTERNAL_");
}

function sortByDateAsc<T extends { date: string }>(items: T[]): T[] {
  return [...items].sort((a, b) => a.date.localeCompare(b.date));
}

function sortByDateDesc<T extends { bookingDate: string }>(items: T[]): T[] {
  return [...items].sort((a, b) => b.bookingDate.localeCompare(a.bookingDate));
}

export async function getNetWorthSeries(): Promise<NetWorthSeries> {
  const snapshot = await loadDatabaseSnapshot();
  const baseCurrency = snapshot?.user_profile.base_currency ?? "EUR";

  if (!snapshot) {
    return {
      baseCurrency,
      points: [],
    };
  }

  const ownAccountIds = new Set(
    snapshot.accounts.filter(isPortfolioAccount).map((account) => account.account_id),
  );

  let runningTotal = 0;
  const dailyTotals = new Map<string, number>();

  for (const transaction of sortByDateAsc(snapshot.transactions)) {
    if (transaction.currency !== baseCurrency) {
      continue;
    }

    let delta = 0;

    if (ownAccountIds.has(transaction.to_account_id)) {
      delta += transaction.amount;
    }

    if (ownAccountIds.has(transaction.from_account_id)) {
      delta -= transaction.amount;
    }

    runningTotal += delta;
    dailyTotals.set(transaction.date, runningTotal);
  }

  return {
    baseCurrency,
    points: [...dailyTotals.entries()].map(([date, amountBaseCcy]) => ({
      date,
      amountBaseCcy,
    })),
  };
}

export async function getAccountBalances(): Promise<AccountBalance[]> {
  const snapshot = await loadDatabaseSnapshot();
  if (!snapshot) {
    return [];
  }

  const baseCurrency = snapshot.user_profile.base_currency;
  const ownAccounts = snapshot.accounts.filter(isPortfolioAccount);
  const ownAccountIds = new Set(ownAccounts.map((account) => account.account_id));
  const balances = new Map<string, number>(ownAccounts.map((account) => [account.account_id, 0]));

  for (const transaction of snapshot.transactions) {
    if (transaction.currency !== baseCurrency) {
      continue;
    }

    if (ownAccountIds.has(transaction.to_account_id)) {
      balances.set(
        transaction.to_account_id,
        (balances.get(transaction.to_account_id) ?? 0) + transaction.amount,
      );
    }

    if (ownAccountIds.has(transaction.from_account_id)) {
      balances.set(
        transaction.from_account_id,
        (balances.get(transaction.from_account_id) ?? 0) - transaction.amount,
      );
    }
  }

  return ownAccounts.map((account) => ({
    accountId: account.account_id,
    accountName: prettyAccountName(account.account_id),
    institution: account.institution,
    currency: baseCurrency,
    balance: balances.get(account.account_id) ?? 0,
  }));
}

export async function getTransactions(): Promise<Transaction[]> {
  const snapshot = await loadDatabaseSnapshot();
  if (!snapshot) {
    return [];
  }

  const transactions = snapshot.transactions.map((transaction) => {
    const sign = transaction.type === "expense" || transaction.type === "fees" ? -1 : 1;

    return {
      id: transaction.txn_id,
      bookingDate: transaction.date,
      accountName: prettyAccountName(transaction.from_account_id),
      description: transaction["description-en"] ?? transaction.description,
      category: transaction.category ?? null,
      amount: transaction.amount * sign,
      currency: transaction.currency,
      enriched: Boolean(transaction["description-en"]),
    };
  });

  return sortByDateDesc(transactions);
}

export async function getParserRuns(): Promise<ParserRun[]> {
  return [];
}
