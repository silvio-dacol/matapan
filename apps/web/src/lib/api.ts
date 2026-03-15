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

type ApiResource = "netWorth" | "accountBalances" | "transactions" | "parserRuns";

const endpointDefaults: Record<ApiResource, string> = {
  netWorth: "/api/v1/net-worth",
  accountBalances: "/api/v1/accounts/balances",
  transactions: "/api/v1/transactions",
  parserRuns: "/api/v1/parser-runs",
};

const endpointEnvKeys: Record<ApiResource, string> = {
  netWorth: "MATAPAN_API_NET_WORTH_PATH",
  accountBalances: "MATAPAN_API_ACCOUNT_BALANCES_PATH",
  transactions: "MATAPAN_API_TRANSACTIONS_PATH",
  parserRuns: "MATAPAN_API_PARSER_RUNS_PATH",
};

function resolveEndpoint(resource: ApiResource): string | null {
  const baseUrl = process.env.MATAPAN_API_BASE_URL?.trim();
  if (!baseUrl) {
    return null;
  }

  const configuredPath = process.env[endpointEnvKeys[resource]]?.trim();
  const path = configuredPath && configuredPath.length > 0 ? configuredPath : endpointDefaults[resource];
  return new URL(path, baseUrl).toString();
}

async function fetchJson(resource: ApiResource): Promise<unknown> {
  const endpoint = resolveEndpoint(resource);
  if (!endpoint) {
    return null;
  }

  const response = await fetch(endpoint, {
    method: "GET",
    headers: {
      Accept: "application/json",
    },
    cache: "no-store",
  });

  if (!response.ok) {
    throw new Error(`Failed to fetch ${resource} from ${endpoint}: HTTP ${response.status}`);
  }

  return response.json();
}

export async function getNetWorthSeries(): Promise<NetWorthSeries> {
  const payload = (await fetchJson("netWorth")) ?? mockNetWorthResponse;
  return parseNetWorthResponse(payload);
}

export async function getAccountBalances(): Promise<AccountBalance[]> {
  const payload = (await fetchJson("accountBalances")) ?? mockAccountBalancesResponse;
  return parseAccountBalancesResponse(payload);
}

export async function getTransactions(): Promise<Transaction[]> {
  const payload = (await fetchJson("transactions")) ?? mockTransactionsResponse;
  return parseTransactionsResponse(payload);
}

export async function getParserRuns(): Promise<ParserRun[]> {
  const payload = (await fetchJson("parserRuns")) ?? mockParserRunsResponse;
  return parseParserRunsResponse(payload);
}
