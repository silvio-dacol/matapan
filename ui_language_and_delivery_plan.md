# UI Language & Delivery Plan for Matapan

## Recommendation (short answer)

Use **TypeScript + React (Next.js)** for both:

- the **net-worth dashboard UI**
- the **data-control console**

and keep your backend/data core in Rust (current repository direction).

This is likely the best balance for your project because:

1. TypeScript gives fast iteration with safer refactors than plain JavaScript.
2. React has the largest ecosystem for charts, tables, forms, and admin tooling.
3. Next.js gives a practical full-stack shell (routing, auth integrations, server actions/API routes) without locking you into a heavy enterprise framework.
4. You can keep Rust where it matters most (parsing, correctness, performance) and expose it via a small API layer.

## Why this stack fits a net-worth product

A personal-finance dashboard and control console usually need:

- rich charting (time-series, allocations, currency splits)
- dense tables with sorting/filtering/export
- strong forms and validation for manual corrections
- role-like UX ("safe mode" for casual use, "advanced mode" for data operators)

TypeScript + React is excellent for this mix and has mature libraries for each part.

## Suggested architecture

- **Frontend app**: Next.js (App Router), TypeScript
- **Design system**: Tailwind CSS + shadcn/ui
- **Charts**: Recharts (fast to start) or ECharts (if you need highly custom interactions)
- **Table/data grid**: TanStack Table
- **Forms**: React Hook Form + Zod
- **API boundary**:
  - Option A (simple): Next.js API routes calling a Rust binary/service
  - Option B (clean separation): Rust service exposing REST/JSON (or gRPC), frontend consumes it

## How to "best obtain" this language/stack

Treat this as "get production-ready quickly, with low rework":

### 1) Team skill acquisition (2-3 weeks)

- Learn only the needed TypeScript subset first:
  - strict typing basics
  - interfaces/types for API payloads
  - discriminated unions for UI states
- Build one vertical slice end-to-end (dashboard card + API call + validation)
- Avoid over-abstracting until 2-3 real screens exist

### 2) Bootstrap a working UI in 1 day

- Create a Next.js TypeScript app
- Add Tailwind and shadcn/ui
- Create routes:
  - `/dashboard`
  - `/console/transactions`
  - `/console/accounts`
  - `/console/rules`

### 3) Build the API contract before full UI

- Define JSON response schema for:
  - net worth over time
  - account balances by currency
  - transaction list with enrichment status
  - parser run/import logs
- Use Zod schema validation on frontend boundaries

### 4) Develop in two tracks

- Track A: dashboard read-only views
- Track B: console write operations (edit/categorize/override)

This reduces risk: dashboard can ship early while console hardening continues.

### 5) Add guardrails early

- optimistic UI only where safe
- audit trail for data edits
- "preview before apply" for destructive operations
- explicit currency conversion source/date shown in UI

## Delivery plan (practical)

### Milestone 1: MVP UI shell (1 week)

- basic layout + navigation
- dashboard cards (net worth, cash flow, top accounts)
- transactions table with filters

### Milestone 2: Data console (1-2 weeks)

- edit transaction category/tag
- manage parsing/cleanup rules
- rerun enrichment and show job status

### Milestone 3: Trust & clarity features (1 week)

- history/audit log
- source drill-down (where each number comes from)
- CSV export for each core table

## If you want an even faster initial path

If your top priority is speed over flexibility, consider:

- **React Admin** for the console
- custom Next.js pages for the dashboard

You can later replace admin pieces with custom UI gradually.

## What not to do initially

- Don’t start with a micro-frontend setup.
- Don’t over-invest in state libraries before you have real complexity.
- Don’t couple chart data shape directly to backend internals; keep a thin adapter layer.

## Final recommendation

For Matapan specifically: **Rust backend + TypeScript/React frontend** is the strongest long-term choice, with the best hiring pool, library ecosystem, and development speed for a data-heavy financial dashboard + operator console.