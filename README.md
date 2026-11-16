# Matapan

Repository following [the MVP code structure](docs/mvp_code_structure.md).
The local account workspace supports account creation, reviewed Revolut CSV
imports, imported transactions with provenance, dated cash reports, and manual
balance confirmation when imported history is incomplete.

For a one-command start that sets up both servers and opens the workspace in the
default browser, run the launcher from the repository root (Python 3.12+ and
Node.js 20.19+ or 22.12+):

```powershell
powershell -ExecutionPolicy Bypass -File .\start-dev.ps1
```

It creates `backend/.venv` and `frontend/node_modules` on first run, applies
pending migrations without touching existing data, and starts both servers.
Press Ctrl+C in its window to stop them.

To start the pieces manually, run the backend using
[backend/README.md](backend/README.md), then open a second
PowerShell terminal from the repository root (Node.js 20.19+ or 22.12+):

```powershell
cd frontend
npm.cmd ci
npm.cmd run dev
```

Open http://127.0.0.1:5173. The Vite server forwards `/api` and `/health` to the
local API at http://127.0.0.1:8000 and preserves the browser's import session cookie.
`npm.cmd` works even when PowerShell blocks the `npm.ps1` wrapper.
See [frontend/README.md](frontend/README.md) for the workflow and browser checks.
For a step-by-step walkthrough, see [the easy frontend guide](docs/easy_frontend_use.md).

- `backend/src/matapan/`: API, database, models, import workflow, Revolut adapter, and reports.
- `backend/migrations/`: Alembic configuration and incremental database migrations.
- `backend/tests/`: API, database, parser, import, and report tests with synthetic fixtures.
- `frontend/src/`: React account workspace, import review, typed API integration, and styles.
- `frontend/tests/`: desktop/mobile browser checks using the real API and disposable synthetic data.
- `.env.example`: reserved for non-secret local settings.

The architecture authority is [docs/mvp_architecture.md](docs/mvp_architecture.md).
The document paths shown in the starting tree are available through
[docs/architecture.md](docs/architecture.md) and
[docs/code-structure.md](docs/code-structure.md), which reference the existing documents.

Keep real bank exports outside Git; tests should use synthetic or anonymized samples.

## Progress recap

Updated **2026-11-11**, including the reconciliation review lifecycle.
Matapan currently provides the first working account-import slice. It is **not yet
the complete MVP** defined by [architecture v0.6](docs/mvp_architecture.md).
The slice names below follow [the implementation guide](docs/mvp_code_structure.md);
they do not imply that all architecture milestones are complete.

### What is implemented

- **Local backend and persistence:** FastAPI, SQLite/SQLAlchemy, exact decimal
  money storage, and incremental Alembic migrations through `0004_review_decisions`.
  Startup checks the migration revision; existing data is
  preserved rather than recreated.
- **Accounts:** create, list, and read accounts; delete empty accounts through the
  API. Accounts with balances, transactions, or import history cannot be deleted.
  The current slice uses one currency per account context, with separate Revolut
  Current and Savings contexts.
- **Revolut CSV import:** one documented English bank-statement format, with
  product/currency mapping, exact amounts and fees, booked movements, original
  source fields, warnings, and dated running-balance evidence. See the
  [supported format and limits](docs/importers/revolut/revolut_csv.md).
- **Reviewed, atomic imports:** preview, refresh, confirm, and cancel; session-bound
  previews expire after 30 minutes. Exact reimports are idempotent. Overlapping
  files are reviewed per row, and ambiguous matches require an explicit decision
  to keep separate, link to an existing transaction, or skip. Multiple source rows
  can retain provenance for one canonical transaction. Commit revalidates matches
  and rolls back financial writes on failure.
- **Dated cash reporting:** account activity and source provenance, manual dated
  balance confirmation, and as-of reports. Missing balances remain unknown;
  future evidence is excluded from past reports. Conflicting observations are
  retained until the user selects which to use; the reason is kept in review
  history. Older evidence is labeled as carried forward. Account cash is partial.
- **Timed checkpoints and reconciliation (Slice 2):** confirm a booked balance
  with a date, time, and UTC offset; explicitly configure the timezone of imported
  statement timestamps. The first timed confirmation provides the baseline, and
  later balances can be calculated from accepted movements after that checkpoint.
  Consecutive checkpoints expose gaps and proposed synthetic "ghost" adjustments.
  Creation, revision, and retirement require approval and preserve evidence and
  audit history. New imports recalculate proposals automatically and mark changed
  approvals as needing review. Zero residual ghosts can be retired on confirmation.
  Ghosts remain separate from real activity and are never added to the confirmed
  balance. A new migration adds these records without replacing existing data.
- **Working frontend:** a React/TypeScript account workspace with account creation,
  statement upload, duplicate and invalid-row review, import confirmation and
  cancellation, paginated activity, dated reports, and manual balance confirmation.
  It now includes import history, reversible row decisions with audit history,
  per-row time correction, and manual movement entry with source reason. The layout
  supports desktop and mobile. This is implemented beyond the
  placeholders mentioned in the original repository guidance.
- **Documentation and checks:** backend/frontend setup instructions, step-by-step
  [API](docs/easy_backend_api_use.md) and
  [frontend](docs/easy_frontend_use.md) guides, synthetic parser/import fixtures,
  backend tests, and Playwright browser tests using a disposable database.

### What still needs to be done

The core of **Slice 2: reconciliation** is implemented. Remaining limitations and
subsequent work, in the guide's suggested order:

1. **Remaining reconciliation limitations:** legacy date-only observations retain
   their original semantics and are not assigned invented times. Imported timed
   calculations require an explicit statement timezone. History coverage stays
   unverified, even when a ghost makes the arithmetic match. Before the first timed
   confirmation the projected balance is unavailable, not zero. The existing dated
   report remains an evidence view; use **Calculate from checkpoint** for
   transaction-derived balances.
2. **Observation and review lifecycle:** improve coverage and reconciliation
   diagnostics so users can see import gaps and understand unresolved intervals.
3. **Brokerage (Slice 3):** verify a representative Interactive Brokers export and
   implement preview/review/import for securities, dated holdings/valuations, and
   separate currency cash balances. Treat aggregate equity/NAV as reconciliation
   evidence so it cannot be counted again alongside cash and positions. No broker
   importer is implemented yet.
4. **Multi-currency net worth (Slice 4):** reporting-currency settings, dated FX with
   explicit source and rounding rules, manual assets/liabilities and their dated
   valuations, and consolidated cash + investments + assets - liabilities. Add
   component drill-down, meaningful historical comparisons, freshness and
   actionable reliability warnings. Current account cash reports are not a
   consolidated net-worth report.
5. **Safe self-hosted release:** private access/authentication appropriate to the
   deployment, Docker/Compose packaging, persistent storage configuration, and
   documented, tested consistent backup/restore. Current development servers bind
   to loopback; production deployment is not complete.
6. **Complete MVP acceptance coverage:** extend tests for the new features,
   including late-import adjustment revision, no double-counted cash/NAV,
   multi-currency valuation, preserved correction history, and restored-report
   reproducibility. Use architecture sections 8, 13.9, and 14.9 as the acceptance
   checklist before declaring the MVP complete.

**Future account organization update:** group accounts by institution in the
frontend (for example, Revolut or SEB), retaining the existing account appearance
within each group. Each account should belong to a reusable institution record.
When creating an account, let the user select an existing institution from a
dropdown or add a new institution without leaving the form. Multiple products
and currencies can belong to the same institution while remaining separate
accounts. Migrate existing institution names and account associations without
losing financial history; review ambiguous name variations rather than silently
merging institutions. Institution membership does not by itself mean that an
importer for that institution is supported. This update is planned, not implemented.

**After the MVP:** additional institution adapters (SEB, Intesa Sanpaolo, China
Construction Bank, Alipay, and WeChat), feature so that people can use it like Splitwise,
broader export variants, transfer linking, categorization, cash-flow/performance analytics, 
inflation, recurring imports, and optional AI. Export-download notes in `docs/importers/` 
do not mean those adapters exist. 
These are later extensions, not prerequisites for the first complete MVP.

### Verification of the current slice

Checks run for this recap on **2026-10-15**:

- Backend: `python -m pytest -q` — **103 passed**, including checkpoint arithmetic,
  ghost revision/retirement, concurrent approvals, rollback, and restore tests.
- Frontend: `npm.cmd run build` — **passed**, including strict TypeScript checks.
- Browser: `npm.cmd test` — **14 passed** across desktop and mobile.
- Ruff lint and format checks passed. Tests also check migration/schema agreement.

The repository includes backend checks for validation, exact money, migrations,
foreign keys, persistence, dated reports, reimports, overlapping files, duplicate
decisions, session expiry, concurrent commits, and rollback. Browser tests cover
the account/import journey and recovery paths on desktop and mobile.

See [backend checks](backend/README.md) and
[frontend checks](frontend/README.md) for commands and environment requirements.
Only synthetic fixtures and disposable test databases should be used for checks.

Verification of the review lifecycle update on **2026-11-11**: backend **107 passed**,
frontend production build and strict TypeScript checks passed, and Ruff lint passed.

# When updating the version

Change both in pyproject.toml and in backend/src/matapan/__init__.py the version of the package. Copy what you have done at work already so you can tag the commit and the releases.
