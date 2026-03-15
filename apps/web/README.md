This is the Matapan web UI built with Next.js.

## Getting Started

Install dependencies and run the development server:

```bash
npm install
npm run dev
```

Open [http://localhost:3000](http://localhost:3000) with your browser.

## Backend Integration

The UI reads data from backend endpoints when `MATAPAN_API_BASE_URL` is configured.
Responses must match the envelope contracts under `src/lib/contracts.ts`.

1. Copy `.env.example` to `.env.local`
2. Set `MATAPAN_API_BASE_URL` to your backend base URL
3. Optionally override endpoint paths with:
	- `MATAPAN_API_NET_WORTH_PATH`
	- `MATAPAN_API_ACCOUNT_BALANCES_PATH`
	- `MATAPAN_API_TRANSACTIONS_PATH`
	- `MATAPAN_API_PARSER_RUNS_PATH`

Default endpoint paths are:

- `/api/v1/net-worth`
- `/api/v1/accounts/balances`
- `/api/v1/transactions`
- `/api/v1/parser-runs`

If `MATAPAN_API_BASE_URL` is not set, the app falls back to local mock data.

## Validation

Run static checks and build:

```bash
npm run lint
npm run build
```
