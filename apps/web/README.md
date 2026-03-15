This is the Matapan web UI built with Next.js.

## Getting Started

Install dependencies and run the development server:

```bash
npm install
npm run dev
```

Open [http://localhost:3000](http://localhost:3000) with your browser.

## Data Source

The UI reads data only from `database/database.json` in this repository.

1. Copy `.env.example` to `.env.local`
2. Optionally set `MATAPAN_DATABASE_PATH` to override the database file path

Default path resolution is:

- `../../database/database.json` (relative to `apps/web`)

If the file is missing or empty, the dashboard shows "No data available yet".

## Validation

Run static checks and build:

```bash
npm run lint
npm run build
```
