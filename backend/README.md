# Matapan backend

The local backend provides FastAPI, SQLite/SQLAlchemy, Alembic migrations, accounts,
dated/timed balance confirmations, reviewed Revolut CSV imports, transaction lists,
manual movements, import history and review corrections, checkpoint projections,
and reviewed reconciliation adjustments.
The [frontend account workspace](../frontend/README.md)
provides account creation, import review, activity, and manual balance confirmation.

From the repository root, run these PowerShell commands (Python 3.12 or newer):

```powershell
cd backend
python -m venv .venv
.\.venv\Scripts\python.exe -m pip install -e ".[dev]"
.\.venv\Scripts\python.exe -m alembic upgrade head
.\.venv\Scripts\python.exe -m uvicorn matapan.main:app --host 127.0.0.1 --port 8000 --reload
```

Open http://127.0.0.1:8000/docs to try the API, or
http://127.0.0.1:8000/health to check that the database is reachable. Keep this local
prototype bound to loopback; authentication and deployment are subsequent work.
The API permits local hostnames and does not enable cross-origin access.

The default database is `backend/data/matapan.sqlite3`, excluded from Git. Startup
checks the migration revision and refuses an unmigrated or outdated database. It
does not create tables automatically or reset existing data. Apply new migrations
with `alembic upgrade head`. The migrations intentionally refuse destructive
downgrades. Migration `0002_revolut_imports` preserves existing manual balances.

An optional `MATAPAN_DATABASE_URL` environment variable selects another SQLite
database. For example, set `$env:MATAPAN_DATABASE_URL = 'sqlite:///./data/test.sqlite3'`
before both the migration and server commands. Relative custom paths resolve from
the command's working directory. `.env` files are not loaded automatically.

Try the following in a second PowerShell terminal while the server is running:

```powershell
$api = 'http://127.0.0.1:8000'
Invoke-RestMethod "$api/health"

$accountBody = @{ name = 'Test account'; currency_code = 'CHF' } | ConvertTo-Json
$account = Invoke-RestMethod "$api/api/v1/accounts" -Method Post -ContentType 'application/json' -Body $accountBody

$balanceBody = @{
    currency_code = 'CHF'
    as_of_date = '2026-10-11'
    amount = '1234.50'
    note = 'Synthetic test balance'
} | ConvertTo-Json
Invoke-RestMethod "$api/api/v1/accounts/$($account.id)/cash-balances" -Method Post -ContentType 'application/json' -Body $balanceBody

Invoke-RestMethod "$api/api/v1/accounts/$($account.id)/report?as_of=2026-10-11"
```

The report should return `amount: "1234.50"`, `balance_status: "user_confirmed"`,
and the observation date. An account with no dated balance returns `amount: null`
and `status: "unavailable"`.

Available endpoints:

| Method | Path | Purpose |
|---|---|---|
| GET | `/health` | API and database health |
| POST / GET | `/api/v1/accounts` | Create / list accounts |
| GET | `/api/v1/accounts/{id}` | Read an account |
| DELETE | `/api/v1/accounts/{id}` | Delete an empty account; linked records return `409` |
| POST / GET | `/api/v1/accounts/{id}/cash-balances` | Confirm / list manual balances |
| POST | `/api/v1/accounts/{id}/balance-resolution` | Select a conflicting balance while preserving all observations |
| GET | `/api/v1/accounts/{id}/report?as_of=YYYY-MM-DD` | Read the latest usable dated cash evidence |
| PUT | `/api/v1/accounts/{id}/statement-timezone` | Explicitly map local export timestamps to an IANA timezone |
| GET | `/api/v1/accounts/{id}/projection?at=ISO_TIMESTAMP` | Calculate from the latest timed confirmed checkpoint |
| GET | `/api/v1/accounts/{id}/reconciliations` | Read interval calculations, proposals, source context, and approval history |
| POST | `/api/v1/reconciliations/{id}/accept-adjustment` | Approve the current proposed ghost amount, including zero retirement |
| POST | `/api/v1/reconciliations/{id}/reverse-adjustment` | Retire an active ghost while retaining its audit history |
| GET | `/api/v1/accounts/{id}/transactions` | Read canonical movements and import provenance |
| POST | `/api/v1/accounts/{id}/transactions/manual` | Add a signed movement with explicit time, source, and reason |
| GET | `/api/v1/accounts/{id}/import-history` | Review prior import batches and row decisions |
| POST | `/api/v1/import-rows/{id}/review` | Revise a committed duplicate decision with a reason |
| GET | `/api/v1/import-rows/{id}/decision-history` | Read append-only row decision history |
| POST | `/api/v1/import-rows/{id}/correct-time` | Correct a row's timestamp and explicit offset while preserving source time |
| POST | `/api/v1/imports/preview` | Upload a CSV and review rows and balance evidence |
| GET | `/api/v1/imports/{id}` | Refresh a preview and its current duplicate candidates |
| POST | `/api/v1/imports/{id}/commit` | Explicitly approve an import plan |
| POST | `/api/v1/imports/{id}/cancel` | Cancel an unconfirmed plan |

List endpoints accept `limit` (1–100, default 100) and `offset` (default 0).
Each initial account has one cash currency context; a balance must match it.
Currency codes are normalized to three uppercase letters; FX conversion and a
currency reference catalog are not implemented in this slice.

`DELETE /api/v1/accounts/{id}` permanently removes an empty account and returns
`204` with no response body. Unknown IDs return `404`. Accounts with balances,
transactions, or import history return `409` and retain all records, including
cancelled or expired import previews. Database foreign keys enforce this protection;
the endpoint never cascades deletion into financial data.

Money must be a JSON **string**, with at most 20 integer digits and 8 fractional
digits. Numeric JSON amounts, non-finite values, and scientific notation are
rejected. SQLAlchemy stores exact `Decimal` values as SQLite text, and the API
returns decimal strings. Dates must use `YYYY-MM-DD`; audit timestamps use UTC.

Balance observations are append-only and record the source, confirmation status,
date, and timestamp. Posting a balance is confirmation of that amount for its
stated date. Conflicting amounts for the latest date are preserved and make the
report unavailable; this slice does not yet offer a conflict-resolution endpoint.
Equal observations retain all source references. For source-reported intraday
snapshots, the latest source timestamp is selected; conflicting values for the
same source checkpoint cannot silently overwrite a saved balance. Older observations are explicitly
marked as carried forward, and future observations are excluded from past reports.
These reports cover account cash only, so known values are marked `partial` rather
than presented as complete net worth.

Run the automated checks from `backend/`:

```powershell
.\.venv\Scripts\python.exe -m pytest
.\.venv\Scripts\python.exe -m ruff check .
.\.venv\Scripts\python.exe -m ruff format --check .
.\.venv\Scripts\python.exe -m alembic check
```

Tests use disposable synthetic databases upgraded through Alembic. They verify
API validation, persistence after restart, exact money storage, foreign keys,
migration safety, dates, conflicts, missing balances, reimports, overlapping files,
duplicate decisions, concurrent commits, and rollback after an interrupted commit.

To try Revolut import in http://127.0.0.1:8000/docs:

1. Create an account with `institution: "Revolut"`, the file's currency, and
   `revolut_product: "Current"` or `"Savings"`. Each product/currency pair needs its
   own account context. The supplied English CSV variant can contain both products.
2. Expand `POST /api/v1/imports/preview`, enter that account's ID, choose the CSV,
   and execute. Review the rows, counts, warnings, and dated balance evidence.
   Other products/currencies are marked `excluded`.
3. Expand `POST /api/v1/imports/{preview_id}/commit` with the returned preview ID.
   For a first import with no ambiguous or invalid rows, the request body is `{}`.
   This is the explicit confirmation; preview alone does not accept financial data.
4. For each `possible_duplicate`, submit either `keep_separate`, `link_duplicate`
   with a listed candidate transaction ID, or `skip`. Invalid rows require `skip`.
   For example:

   ```json
   {
     "decisions": [
       {
         "row_id": "the-preview-row-UUID",
         "action": "link_duplicate",
         "transaction_id": "the-listed-candidate-UUID"
       }
     ],
     "include_balance": true
   }
   ```

5. Query the account's transactions and dated report. Imported amounts are net
   movements (`Amount - Fee`); original amount, fee, immutable source description,
   and links to source rows remain available.

Previews expire after 30 minutes and are bound to the browser's HTTP-only session
cookie. Use the same browser or preserve cookies in your HTTP client. Cancelled or
expired plans can be renewed by uploading the file again. If another import adds
matching records, commit returns `409` until you refresh and review the candidates.
Repeat commits and exact-file reuploads cannot create duplicate movements.
Files with overlapping dates are reviewed per row; their date ranges are not a
reason to reject the whole file.

A conflicting source balance blocks acceptance of that balance. Set
`include_balance: false` to explicitly accept reviewed movements while preserving
the existing balance. This does not resolve the conflicting evidence.
Normalized preview/source rows are retained in private local SQLite; original CSV
bytes and full upload paths are not retained. This slice keeps only the latest
usable running-balance checkpoint per file/account context as a canonical balance.
It never labels the filename's end date as a verified closing balance.

See [the supported Revolut format](docs/revolut_csv.md) for parsing and timestamp
semantics. The original private sample files are outside the repository; committed
fixtures contain invented descriptions and amounts.

## Timed checkpoints and ghost adjustments

Apply migration `0003_reconciliation` with `alembic upgrade head` after installing
updated dependencies (`pip install -e ".[dev]"`). It adds nullable timestamp fields
and new reconciliation tables. Existing dates, amounts, imports, and provenance
remain intact. `tzdata` supplies IANA timezone rules on Windows.

To establish a timed reference, post a balance with the existing fields plus
`"effective_at": "2026-10-15T14:30:00+02:00"`. `as_of_date` must match the local
date in that timestamp. An explicit offset and whole seconds are required. The API
stores the instant in UTC and preserves the entered offset separately from
`recorded_at`. Confirm the booked cash balance including all booked movements up
to that instant, excluding pending movements. Date-only observations remain
supported and are never assigned an invented time.

Set `{"statement_timezone": "Europe/Zurich"}` using the timezone endpoint only
after verifying the timezone used in the source export. The adapter retains its
original local timestamps. Missing timezone mapping and ambiguous/nonexistent
daylight-saving times block calculations rather than guessing a transaction's
instant. Per-row time correction is not implemented yet.

Projection uses the latest nonconflicted timed manual checkpoint on/before the
requested instant and each accepted canonical booked movement in
`(checkpoint instant, requested instant]` once. No checkpoint means unavailable,
not zero. Original observations remain separate from calculated values. Earlier
movements and ghosts are already reflected in the selected checkpoint and are
never added again. URL-encode the `at` value, especially a positive UTC offset.
The existing date-only report continues to select evidence; it does not silently
switch to a movement-derived figure.

Consecutive timed checkpoints define nonoverlapping reconciliation intervals:

```text
required ghost = confirmed close - (confirmed open + accepted real movements)
```

The list endpoint returns opening/closing evidence, movement count/total, current
ghost, remaining difference, required ghost, source batch/decision context, issues,
and a `signature`. Approve with `{"signature": "the reviewed signature", "reason":
"Missing statement interval"}`. The server recalculates under SQLite's writer lock;
stale signatures return `409`. Approval creates an audited version, superseding
the old one. Ghosts are separate records, excluded from real transaction activity
and income/expense. Their effective boundary is the interval's closing checkpoint.

After new imports or timezone changes, proposals are recalculated on read. Changed
approved evidence becomes `needs_review`; saved amounts do not change until
approval. A zero proposal retires the ghost on confirmation. Voluntary reversal
also records a decision; if a discrepancy remains, the interval is unresolved.
Original amounts, reasons, approvals, and decision-time evidence are retained.
Inserting a checkpoint splits the interval: retire any old spanning ghost before
approving adjustments for the new intervals. Conflicting confirmations at one
instant block approval pending future conflict-resolution support.

Coverage remains unverified even when the arithmetic agrees. A source-backed
checkpoint remains valid for its instant while an adjustment awaits review.
Broker holdings, FX, consolidated net worth, conflict resolution, authentication,
and deployment remain subsequent work required by the
[architecture](../docs/mvp_architecture.md); this is not the complete MVP.
