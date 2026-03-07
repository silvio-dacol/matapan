import {
  parseAccountBalancesResponse,
  parseNetWorthResponse,
  parseParserRunsResponse,
  parseTransactionsResponse,
  type AccountBalance,
  type NetWorthSeries,
  type ParserRun,
  type Transaction,
} from "@/lib/contracts";
import {
  mockAccountBalancesResponse,
  mockNetWorthResponse,
  mockParserRunsResponse,
  mockTransactionsResponse,
} from "@/lib/mock-data";

export async function getNetWorthSeries(): Promise<NetWorthSeries> {
  return parseNetWorthResponse(mockNetWorthResponse);
}

export async function getAccountBalances(): Promise<AccountBalance[]> {
  return parseAccountBalancesResponse(mockAccountBalancesResponse);
}

export async function getTransactions(): Promise<Transaction[]> {
  return parseTransactionsResponse(mockTransactionsResponse);
}

export async function getParserRuns(): Promise<ParserRun[]> {
  return parseParserRunsResponse(mockParserRunsResponse);
}
