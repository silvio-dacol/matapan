// Money is always a decimal string. The server owns all financial calculations.
export interface Account {
  id: string;
  name: string;
  institution: string | null;
  currency_code: string;
  revolut_product: "Current" | "Savings" | null;
  created_at: string;
  statement_timezone: string | null;
}

export type AccountCreate = Pick<
  Account,
  "name" | "institution" | "currency_code" | "revolut_product"
>;

export interface Balance {
  id: string;
  account_id: string;
  currency_code: string;
  as_of_date: string;
  amount: string;
  source: "manual" | "revolut";
  status: "user_confirmed" | "source_reported";
  source_timestamp: string | null;
  note: string | null;
  recorded_at: string;
  effective_at: string | null;
  effective_offset: string | null;
}

export interface ImportHistoryBatch {
  id: string;
  status: string;
  created_at: string;
  committed_at: string | null;
  observed_start: string | null;
  observed_end: string | null;
  parser_id: string;
  warnings: string[];
  rows: { id: string; row_number: number; classification: string;
    decision: string | null; transaction_id: string | null;
    description: string; amount: string | null; posted_at: string | null;
    corrected_posted_at: string | null; corrected_offset: string | null;
    candidates: { id: string; description: string; amount: string }[] }[];
}

export interface AccountReport {
  account_id: string;
  currency_code: string;
  as_of_date: string;
  scope: "account_cash";
  status: "partial" | "unavailable";
  amount: string | null;
  balance_as_of_date: string | null;
  balance_status:
    "user_confirmed" | "source_reported" | "missing" | "conflicted";
  source: "manual" | "revolut" | null;
  source_timestamp: string | null;
  balance_effective_at: string | null;
  observation_ids: string[];
  carried_forward: boolean;
  issues: string[];
  resolution: { id: string; selected_balance_id: string; reason: string } | null;
}

export interface Transaction {
  id: string;
  account_id: string;
  currency_code: string;
  product: string;
  transaction_type: string;
  posted_at: string;
  posted_offset: string | null;
  started_at: string;
  amount: string;
  source_amount: string;
  fee: string;
  source_description: string;
  import_row_ids: string[];
  source: "import" | "manual";
  manual_reason: string | null;
}

export type Classification =
  | "new"
  | "duplicate"
  | "possible_duplicate"
  | "invalid"
  | "excluded"
  | "conflict";
export interface PreviewRow {
  id: string;
  row_number: number;
  classification: Classification;
  product: string;
  currency_code: string;
  transaction_type: string;
  source_state: string;
  description: string;
  posted_at: string | null;
  started_at: string | null;
  amount: string | null;
  source_amount: string | null;
  fee: string | null;
  errors: string[];
  candidate_transactions: {
    id: string;
    posted_at: string;
    amount: string;
    description: string;
  }[];
  decision: string | null;
  transaction_id: string | null;
  balance_id: string | null;
}

export interface ImportPreview {
  id: string;
  account_id: string;
  currency_code: string;
  product: string;
  parser_id: string;
  parser_version: string;
  status: "preview" | "committed" | "cancelled";
  already_imported: boolean;
  expires_at: string;
  observed_start: string | null;
  observed_end: string | null;
  declared_period: null;
  counts: Partial<Record<Classification, number>>;
  warnings: string[];
  balance: {
    classification: "new" | "duplicate" | "conflict";
    amount: string;
    as_of_date: string;
    source_timestamp: string;
    row_id: string;
    existing_balance_id: string | null;
    source: "revolut";
    status: "source_reported";
  } | null;
  rows: PreviewRow[];
}

export interface RowDecision {
  row_id: string;
  action: "keep_separate" | "link_duplicate" | "skip";
  transaction_id?: string;
}

export interface CommitResult {
  id: string;
  status: string;
  new_transactions: number;
  transactions_in_batch: number;
  linked_duplicates: number;
  skipped_rows: number;
  balance_id: string | null;
  already_imported: boolean;
}

export interface Checkpoint {
  id: string;
  amount: string;
  effective_at: string;
  effective_offset: string;
}

export interface Reconciliation {
  id: string;
  currency_code: string;
  opening: Checkpoint;
  closing: Checkpoint;
  state: "reconciled" | "unresolved" | "needs_review" | "blocked" | "superseded";
  current: boolean;
  movement_count: number;
  real_total: string;
  expected_close: string | null;
  required_adjustment: string | null;
  active_adjustment: string;
  difference: string | null;
  signature: string;
  issues: string[];
  evidence: {
    imports: { id: string; observed_start: string | null; observed_end: string | null;
      warnings: string[]; decisions: { row_id: string; action: string; transaction_id: string | null }[] }[];
    pending_rows: number;
    excluded_rows: number;
    statement_timezone: string | null;
  };
  history: { id: string; amount: string; state: string; action: string; reason: string;
    approved_at: string; actor: string; replaces_id: string | null }[];
}

export interface Projection {
  at: string;
  amount: string | null;
  status: "confirmed" | "calculated" | "unavailable";
  checkpoint: Checkpoint | null;
  real_total: string;
  movement_count: number;
  issues: string[];
}

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  let response: Response;
  try {
    response = await fetch(path, { ...options, credentials: "same-origin" });
  } catch {
    throw new ApiError(
      0,
      "Matapan could not connect. Check that the local API is running, then retry.",
    );
  }
  const content = (await response.json().catch(() => null)) as {
    detail?: unknown;
  } | null;
  if (!response.ok) {
    let message =
      "The request could not be completed. Check that the local API is running.";
    if (typeof content?.detail === "string") message = content.detail;
    if (Array.isArray(content?.detail)) {
      message = content.detail
        .map(
          (item: { loc?: string[]; msg?: string }) =>
            `${item.loc?.slice(1).join(" · ") || "Request"}: ${item.msg || "Invalid value"}`,
        )
        .join("; ");
    }
    throw new ApiError(response.status, message);
  }
  return content as T;
}

const json = (body: unknown): RequestInit => ({
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify(body),
});

export const api = {
  health: () => request<{ status: "ok"; version: string }>("/health"),
  accounts: async () => {
    const accounts: Account[] = [];
    for (let offset = 0; ; offset += 100) {
      const page = await request<Account[]>(
        `/api/v1/accounts?limit=100&offset=${offset}`,
      );
      accounts.push(...page);
      if (page.length < 100) return accounts;
    }
  },
  createAccount: (body: AccountCreate) =>
    request<Account>("/api/v1/accounts", json(body)),
  report: (id: string, date: string) =>
    request<AccountReport>(`/api/v1/accounts/${id}/report?as_of=${date}`),
  transactions: (id: string, offset = 0) =>
    request<Transaction[]>(
      `/api/v1/accounts/${id}/transactions?limit=25&offset=${offset}`,
    ),
  balances: (id: string, offset = 0) =>
    request<Balance[]>(
      `/api/v1/accounts/${id}/cash-balances?limit=10&offset=${offset}`,
    ),
  resolveBalance: (id: string, selected_balance_id: string, reason: string) =>
    request<AccountReport>(`/api/v1/accounts/${id}/balance-resolution`,
      json({ selected_balance_id, reason })),
  importHistory: (id: string) =>
    request<ImportHistoryBatch[]>(`/api/v1/accounts/${id}/import-history`),
  reviewImportRow: (id: string, action: "keep_separate" | "link_duplicate" | "skip",
    reason: string, transaction_id?: string) =>
    request<{ row_id: string; decision: string; transaction_id: string | null }>(
      `/api/v1/import-rows/${id}/review`,
      json({ action, reason, ...(transaction_id ? { transaction_id } : {}) })),
  importRowDecisionHistory: (id: string) =>
    request<{ id: string; previous_action: string | null; previous_transaction_id: string | null;
      action: string; transaction_id: string | null; reason: string; created_at: string }[]>(
      `/api/v1/import-rows/${id}/decision-history`),
  correctImportTime: (id: string, posted_at: string, reason: string) =>
    request<{ row_id: string; corrected_posted_at: string; corrected_offset: string }>(
      `/api/v1/import-rows/${id}/correct-time`, json({ posted_at, reason })),
  confirmBalance: (
    id: string,
    body: {
      currency_code: string;
      as_of_date: string;
      amount: string;
      note: string | null;
      effective_at?: string;
    },
  ) => request<Balance>(`/api/v1/accounts/${id}/cash-balances`, json(body)),
  createManualMovement: (id: string, body: { posted_at: string; amount: string;
    description: string; reason: string }) =>
    request<Transaction>(`/api/v1/accounts/${id}/transactions/manual`, json(body)),
  statementTimezone: (id: string, zone: string) =>
    request<Account>(`/api/v1/accounts/${id}/statement-timezone`, {
      ...json({ statement_timezone: zone }), method: "PUT",
    }),
  reconciliations: (id: string) =>
    request<Reconciliation[]>(`/api/v1/accounts/${id}/reconciliations`),
  adjustment: (id: string, signature: string, reason: string, reverse = false) =>
    request<Reconciliation>(`/api/v1/reconciliations/${id}/${reverse ? "reverse" : "accept"}-adjustment`,
      json({ signature, reason })),
  projection: (id: string, at: string) =>
    request<Projection>(`/api/v1/accounts/${id}/projection?at=${encodeURIComponent(at)}`),
  preview: (id: string, file: File) => {
    const body = new FormData();
    body.append("account_id", id);
    body.append("file", file);
    return request<ImportPreview>("/api/v1/imports/preview", {
      method: "POST",
      body,
    });
  },
  refreshPreview: (id: string) =>
    request<ImportPreview>(`/api/v1/imports/${id}`),
  commit: (id: string, decisions: RowDecision[], includeBalance: boolean) =>
    request<CommitResult>(
      `/api/v1/imports/${id}/commit`,
      json({ decisions, include_balance: includeBalance }),
    ),
  cancel: (id: string) =>
    request<{ status: string }>(`/api/v1/imports/${id}/cancel`, {
      method: "POST",
    }),
};

export function errorMessage(error: unknown): string {
  return error instanceof Error
    ? error.message
    : "Something went wrong. Please retry.";
}

// Format the decimal text directly, preserving all digits without Number/parseFloat.
export function money(amount: string | null, currency: string): string {
  if (amount === null) return "Not available";
  const negative = amount.startsWith("-");
  const [integer, fraction = ""] = amount.replace(/^-/, "").split(".");
  return `${negative ? "−" : ""}${integer.replace(/\B(?=(\d{3})+(?!\d))/g, ",")}.${fraction.padEnd(2, "0")} ${currency}`;
}

export function calendarDate(value: string | null): string {
  if (!value) return "No date";
  // Provider timestamps have no timezone; retain their calendar date.
  return new Intl.DateTimeFormat("en-GB", {
    day: "numeric",
    month: "short",
    year: "numeric",
    timeZone: "UTC",
  }).format(new Date(`${value.slice(0, 10)}T12:00:00Z`));
}

export function today(): string {
  const date = new Date();
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

export function localOffset(): string {
  const minutes = -new Date().getTimezoneOffset();
  return `${minutes < 0 ? "-" : "+"}${String(Math.floor(Math.abs(minutes) / 60)).padStart(2, "0")}:${String(Math.abs(minutes) % 60).padStart(2, "0")}`;
}

export function checkpointTime(checkpoint: Checkpoint): string {
  return `${new Intl.DateTimeFormat("en-GB", { dateStyle: "medium", timeStyle: "medium", timeZone: checkpoint.effective_offset === "+00:00" ? "UTC" : checkpoint.effective_offset }).format(new Date(checkpoint.effective_at))} UTC${checkpoint.effective_offset}`;
}
