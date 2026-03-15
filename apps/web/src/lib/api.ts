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

export async function getNetWorthSeries(): Promise<NetWorthSeries> {
  const snapshot = await loadDatabaseSnapshot();
  const baseCurrency = snapshot?.user_profile.base_currency ?? "EUR";

  return {
    baseCurrency,
    points: [],
  };
}

export async function getAccountBalances(): Promise<AccountBalance[]> {
  const snapshot = await loadDatabaseSnapshot();
  if (!snapshot) {
    return [];
  }

  return snapshot.accounts.map((account) => ({
    accountId: account.account_id,
    accountName: prettyAccountName(account.account_id),
    institution: account.institution,
    currency: snapshot.user_profile.base_currency,
    balance: 0,
  }));
}

export async function getTransactions(): Promise<Transaction[]> {
  const snapshot = await loadDatabaseSnapshot();
  if (!snapshot) {
    return [];
  }

  return snapshot.transactions.map((transaction) => {
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
}

export async function getParserRuns(): Promise<ParserRun[]> {
  return [];
}
