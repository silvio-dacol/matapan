# Matapan account workspace

The React/TypeScript frontend connects to the existing FastAPI backend. It uses a
warm light palette with dark green actions and adapts to desktop and mobile.

For a step-by-step walkthrough, see
[the easy frontend guide](../docs/easy_frontend_use.md).

Start the backend and apply its migrations as described in
[backend/README.md](../backend/README.md). With Node.js 20.19+ or 22.12+ installed,
run from this directory:

```powershell
npm.cmd ci
npm.cmd run dev
```

Open http://127.0.0.1:5173. Keep the backend on http://127.0.0.1:8000. Vite proxies
API requests so previews, refreshes, confirmation, and cancellation share the
HTTP-only session cookie. CORS changes are unnecessary. `MATAPAN_API_TARGET` can
select a different local backend address before starting Vite.

## First account journey

1. Create an account with its currency. For Revolut, select Current or Savings;
   each product/currency pair needs a separate account.
2. Select **Import statement**, choose a supported English Revolut CSV, and
   preview it. Review observed transaction dates, counts, source balance evidence,
   and warnings. The preview does not accept movements or balances.
3. Compare existing candidates for possible duplicates. Choose **Keep as a
   separate movement**, **Link to existing match**, or **Skip this row**. Invalid
   rows require an explicit skip; rows from other products/currencies are excluded.
4. Approve the decisions and selected balance evidence, then **Confirm import**.
   The account report and activity refresh. Reimporting the same file cannot add
   its movements again.
5. If history is incomplete or a balance is missing, select **Confirm balance**.
   Enter the actual amount, date, optional note, and explicit confirmation.
   Monetary input uses decimal text, never floating-point conversions.
6. Use **Add movement** for a manually recorded signed transaction. Supply its
   date/time, UTC offset, description, and reason. A matching later import appears
   for duplicate review; linking it keeps both the manual and import provenance.
7. Use **Import history** to inspect prior batches. Revise row decisions with a
   reason, inspect their audit trail, or correct an ambiguous row time with an
   explicit UTC offset. The original imported timestamp remains preserved.

The report date selects dated evidence through the backend. The activity list
shows all imported dates with pagination. Balance evidence retains historical
observations and confirmations, including observations after the report date.
Both account selection and adding an account are disabled during a form or import
review to avoid changing the destination account midway through the work.

## What the figures mean

- **Confirmed by you** identifies a dated manual confirmation.
- **Source reported** identifies a Revolut running balance at its original local
  timestamp; it is not presented as a verified end-of-day balance.
- **Conflicting evidence** leaves the amount unavailable and preserves all
  observations. Select an observation from **Balance evidence** and record why;
  the decision itself is retained for later review.
- An earlier observation is explicitly carried forward, rather than described
  as confirmed for a later date. Missing evidence is unavailable, never zero.
- History coverage remains unverified even after a balance confirmation. The UI
  does not compute balances from an arbitrary subset of movements.

## Timed reference and reconciliation workflow

Update backend dependencies and run `alembic upgrade head` before starting the API.
In **Confirm balance**, check **Use this balance as a timed reference checkpoint**,
then enter the time and the UTC offset that applied on that date. Confirm booked
cash including movements up to that instant and excluding pending movements.
Old date-only confirmations retain their original meaning.

In **Balance reconciliation**, explicitly confirm the timezone used by your source
statement's timestamps. **Calculate from checkpoint** shows a derived balance from
the latest timed checkpoint and subsequent accepted movements, without changing
the confirmed amount. The account cash card remains a dated evidence view.

Two timed checkpoints expose an interval with its real movements, expected balance,
remaining difference, and proposed ghost amount. Review coverage, duplicate
decisions, and pending/skipped rows; enter a reason, check the approval box, and
confirm the proposal. To investigate, leave it unresolved and obtain missing data.

Late imports automatically recalculate the proposal and mark changed approvals as
needing review. Confirm a replacement or a zero proposal to retire the old ghost.
**Retire approved ghost** also supports voluntary reversal. Audit history remains;
ghosts are never listed as imported real transactions or added to a confirmed
balance. A newly inserted intermediate checkpoint requires retirement of an old
spanning ghost before reviewing the new intervals.

Missing timezone mappings, ambiguous/nonexistent daylight-saving times, or conflicting
confirmations at one instant block the affected calculations. Correct an ambiguous
import row from **Import history**, with a verified local time and offset. Coverage
remains unverified even after accepting a ghost. Brokerage, FX, consolidated net
worth, authentication, and self-hosted production packaging remain subsequent work.
The development and preview servers bind to loopback.

## Verification

```powershell
npm.cmd run build
npm.cmd test
```

The build includes strict TypeScript checks. Playwright starts Vite on port 5174
and a real API on port 8001, using a newly migrated disposable database under the
ignored `.test-data/` directory. It refuses to reuse running servers on these ports
and never points at the user's financial database. Tests use the backend's
synthetic CSV fixtures. No original bank exports are needed.

Install the backend's development dependencies first. Browser tests default to
`backend/.venv/Scripts/python.exe` on Windows and `backend/.venv/bin/python`
elsewhere. Set `MATAPAN_TEST_PYTHON` to use another Python environment with Matapan
installed. Windows tests use installed Microsoft Edge in headless mode. On other
platforms, install Playwright's Chromium with `npx playwright install chromium`.

Desktop/mobile tests cover timed checkpoints, ghost revision and zero retirement,
checkpoint projections, preview versus commit, exact reimport, reviewed
overlaps and source provenance, invalid-row decisions, cancellation, exact large
amounts, dated confirmation, past reports, conflicting balances, persistence after
reload, preview expiry, concurrent-import recovery, and connection recovery.
Screenshots and failure traces stay in ignored
`test-results/`. Do not use private financial data for those artifacts.

`npm.cmd run preview` serves the built frontend locally with the same API proxy.
Production hosting needs a same-origin API reverse proxy; the generated static
assets alone do not include or replace the backend.
