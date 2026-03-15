import { z } from "zod";

const versionSchema = z.literal("v1");
const isoDateTimeSchema = z.string().datetime();
const currencySchema = z.string().regex(/^[A-Z]{3}$/);

const envelope = <T extends z.ZodTypeAny>(dataSchema: T) =>
  z.object({
    version: versionSchema,
    generatedAt: isoDateTimeSchema,
    data: dataSchema,
  });

export const netWorthPointSchema = z.object({
  date: z.string().date(),
  amountBaseCcy: z.number(),
});

export const netWorthSeriesSchema = z.object({
  baseCurrency: currencySchema,
  points: z.array(netWorthPointSchema),
});

export const accountBalanceSchema = z.object({
  accountId: z.string(),
  accountName: z.string(),
  institution: z.string(),
  currency: currencySchema,
  balance: z.number(),
});

export const transactionSchema = z.object({
  id: z.string(),
  bookingDate: z.string().date(),
  accountName: z.string(),
  description: z.string(),
  category: z.string().nullable(),
  amount: z.number(),
  currency: currencySchema,
  enriched: z.boolean(),
});

export const parserRunSchema = z.object({
  runId: z.string(),
  source: z.string(),
  status: z.enum(["queued", "running", "succeeded", "failed"]),
  startedAt: isoDateTimeSchema,
  completedAt: isoDateTimeSchema.nullable(),
  importedRows: z.number().int().nonnegative(),
});

export const netWorthResponseSchema = envelope(netWorthSeriesSchema);
export const accountBalancesResponseSchema = envelope(z.array(accountBalanceSchema));
export const transactionsResponseSchema = envelope(z.array(transactionSchema));
export const parserRunsResponseSchema = envelope(z.array(parserRunSchema));

export const databaseUserProfileSchema = z.object({
  base_currency: currencySchema,
});

export const databaseAccountSchema = z.object({
  account_id: z.string(),
  institution: z.string(),
});

export const databaseTransactionSchema = z.object({
  txn_id: z.string(),
  date: z.string().date(),
  from_account_id: z.string(),
  to_account_id: z.string(),
  type: z.string(),
  category: z.string().nullable().optional(),
  amount: z.number(),
  currency: currencySchema,
  description: z.string(),
  "description-en": z.string().optional(),
});

export const databaseSchema = z.object({
  user_profile: databaseUserProfileSchema,
  accounts: z.array(databaseAccountSchema).default([]),
  transactions: z.array(databaseTransactionSchema).default([]),
  positions: z.array(z.unknown()).default([]),
});

export type NetWorthSeries = z.infer<typeof netWorthSeriesSchema>;
export type AccountBalance = z.infer<typeof accountBalanceSchema>;
export type Transaction = z.infer<typeof transactionSchema>;
export type ParserRun = z.infer<typeof parserRunSchema>;
export type DatabaseSnapshot = z.infer<typeof databaseSchema>;

export function parseNetWorthResponse(input: unknown): NetWorthSeries {
  return netWorthResponseSchema.parse(input).data;
}

export function parseAccountBalancesResponse(input: unknown): AccountBalance[] {
  return accountBalancesResponseSchema.parse(input).data;
}

export function parseTransactionsResponse(input: unknown): Transaction[] {
  return transactionsResponseSchema.parse(input).data;
}

export function parseParserRunsResponse(input: unknown): ParserRun[] {
  return parserRunsResponseSchema.parse(input).data;
}

export function parseDatabaseSnapshot(input: unknown): DatabaseSnapshot {
  return databaseSchema.parse(input);
}
