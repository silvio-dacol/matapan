export const mockNetWorthResponse = {
  version: "v1",
  generatedAt: "2026-03-07T09:18:00.000Z",
  data: {
    baseCurrency: "EUR",
    points: [
      { date: "2026-03-03", amountBaseCcy: 309410.22 },
      { date: "2026-03-04", amountBaseCcy: 311030.68 },
      { date: "2026-03-05", amountBaseCcy: 314208.04 },
      { date: "2026-03-06", amountBaseCcy: 313785.77 },
      { date: "2026-03-07", amountBaseCcy: 315190.9 },
    ],
  },
};

export const mockAccountBalancesResponse = {
  version: "v1",
  generatedAt: "2026-03-07T09:18:00.000Z",
  data: [
    {
      accountId: "acc-revolut-main",
      accountName: "Revolut Main",
      institution: "Revolut",
      currency: "EUR",
      balance: 18342.2,
    },
    {
      accountId: "acc-ibkr-broker",
      accountName: "Interactive Brokers",
      institution: "IBKR",
      currency: "USD",
      balance: 136210.52,
    },
    {
      accountId: "acc-intesa-cash",
      accountName: "Intesa Cash",
      institution: "Intesa Sanpaolo",
      currency: "EUR",
      balance: 26940.11,
    },
  ],
};

export const mockTransactionsResponse = {
  version: "v1",
  generatedAt: "2026-03-07T09:18:00.000Z",
  data: [
    {
      id: "tx-40403",
      bookingDate: "2026-03-07",
      accountName: "Revolut Main",
      description: "Groceries - Via Santa Marta",
      category: "Food",
      amount: -82.4,
      currency: "EUR",
      enriched: true,
    },
    {
      id: "tx-40412",
      bookingDate: "2026-03-07",
      accountName: "Interactive Brokers",
      description: "Dividend NVDA",
      category: null,
      amount: 124.18,
      currency: "USD",
      enriched: false,
    },
    {
      id: "tx-40417",
      bookingDate: "2026-03-06",
      accountName: "Intesa Cash",
      description: "Mortgage Payment",
      category: "Housing",
      amount: -920,
      currency: "EUR",
      enriched: true,
    },
  ],
};

export const mockParserRunsResponse = {
  version: "v1",
  generatedAt: "2026-03-07T09:18:00.000Z",
  data: [
    {
      runId: "run-5513",
      source: "interactive_brokers",
      status: "succeeded",
      startedAt: "2026-03-07T08:55:00.000Z",
      completedAt: "2026-03-07T08:55:22.000Z",
      importedRows: 242,
    },
    {
      runId: "run-5514",
      source: "revolut",
      status: "running",
      startedAt: "2026-03-07T09:11:18.000Z",
      completedAt: null,
      importedRows: 0,
    },
  ],
};
