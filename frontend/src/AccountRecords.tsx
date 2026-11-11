import { useEffect, useState } from "react";
import { api, calendarDate, errorMessage, money } from "./api";
import type { Account, AccountReport, Balance, ImportHistoryBatch, Transaction } from "./api";
import { Notice } from "./Notice";

export function Activity({
  account,
  revision,
}: {
  account: Account;
  revision: number;
}) {
  const [offset, setOffset] = useState(0);
  const [rows, setRows] = useState<Transaction[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    setOffset(0);
  }, [revision]);
  useEffect(() => {
    let active = true;
    setLoading(true);
    setError("");
    api
      .transactions(account.id, offset)
      .then((result) => {
        if (active) setRows(result);
      })
      .catch((failure) => {
        if (active) {
          setRows([]);
          setError(errorMessage(failure));
        }
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [account.id, offset, revision, retry]);

  return (
    <section className="panel activity-panel" aria-labelledby="activity-title">
      <div className="section-heading">
        <div>
          <span className="eyebrow">YOUR IMPORTED RECORDS</span>
          <h2 id="activity-title">Account activity</h2>
        </div>
        <span className="tag">All dates</span>
      </div>
      <p className="muted compact">
        Accepted movements, with their original source details. This list may
        cover only part of your history.
      </p>
      {error ? (
        <Notice error>
          {error}{" "}
          <button onClick={() => setRetry((value) => value + 1)}>
            Retry activity
          </button>
        </Notice>
      ) : loading ? (
        <p className="loading" role="status">
          Loading activity…
        </p>
      ) : rows.length === 0 ? (
        <div className="empty-state">
          <span className="empty-symbol" aria-hidden="true">
            ↔
          </span>
          <h3>
            {offset
              ? "No more transactions"
              : "Your activity starts with a statement"}
          </h3>
          <p>
            {offset
              ? "Go back to see the previous records."
              : "Import a Revolut CSV to review and save your transactions. A confirmed balance can stand on its own."}
          </p>
        </div>
      ) : (
        <div className="table-scroll">
          <table>
            <caption className="sr-only">
              Imported account transactions, all dates
            </caption>
            <thead>
              <tr>
                <th>Date</th>
                <th>Transaction</th>
                <th className="numeric">Amount</th>
                <th>Evidence</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.id}>
                  <td className="nowrap">
                    {calendarDate(row.posted_at)}
                    <span className="cell-subtitle">
                      {row.posted_at.slice(11, 16)} · {row.source === "manual" ? `manual time UTC${row.posted_offset ?? ""}` : "source local time"}
                    </span>
                  </td>
                  <td>
                    <span className="transaction-description">
                      {row.source_description || "No description"}
                    </span>
                    <span className="cell-subtitle">
                      {row.transaction_type} · {row.product} · {row.source === "manual" ? "manual provenance" : "imported"}
                    </span>
                  </td>
                  <td
                    className={`numeric amount ${row.amount.startsWith("-") ? "" : "positive"}`}
                  >
                    {money(row.amount, row.currency_code)}
                  </td>
                  <td>
                    <details>
                      <summary>
                        {row.source === "manual" ? "Manual source" : `${row.import_row_ids.length} source ${row.import_row_ids.length === 1 ? "row" : "rows"}`}
                      </summary>
                      <div className="evidence-details">
                        <p>
                          Source amount:{" "}
                          {money(row.source_amount, row.currency_code)}
                        </p>
                        <p>Fee: {money(row.fee, row.currency_code)}</p>
                        <p>Net movement includes the fee.</p>
                        {row.manual_reason && <p>Reason: {row.manual_reason}</p>}
                        {row.import_row_ids.map((id) => (
                          <code key={id}>{id}</code>
                        ))}
                      </div>
                    </details>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <div className="pagination">
        <span className="muted">
          {!loading && rows.length > 0
            ? `Records ${offset + 1}–${offset + rows.length}`
            : "25 records per page"}
        </span>
        <div>
          <button
            disabled={loading || offset === 0}
            onClick={() => setOffset((value) => Math.max(0, value - 25))}
          >
            Previous
          </button>
          <button
            disabled={loading || rows.length < 25}
            onClick={() => setOffset((value) => value + 25)}
          >
            Next
          </button>
        </div>
      </div>
    </section>
  );
}

export function BalanceHistory({
  account,
  revision,
  report,
  onResolved,
}: {
  account: Account;
  revision: number;
  report: AccountReport | null;
  onResolved: () => void;
}) {
  const [offset, setOffset] = useState(0);
  const [balances, setBalances] = useState<Balance[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [retry, setRetry] = useState(0);
  const [reason, setReason] = useState("");
  const [resolving, setResolving] = useState(false);
  useEffect(() => {
    setOffset(0);
  }, [revision]);
  useEffect(() => {
    let active = true;
    setLoading(true);
    setError("");
    api
      .balances(account.id, offset)
      .then((result) => {
        if (active) setBalances(result);
      })
      .catch((failure) => {
        if (active) setError(errorMessage(failure));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [account.id, revision, offset, retry]);
  return (
    <section className="panel" aria-labelledby="evidence-title">
      <div className="section-heading">
        <div>
          <span className="eyebrow">PRESERVED OBSERVATIONS</span>
          <h2 id="evidence-title">Balance evidence</h2>
        </div>
      </div>
      <p className="muted compact">
        Original balances and your confirmations, including observations after
        the report date.
      </p>
      {error ? (
        <Notice error>
          {error}{" "}
          <button onClick={() => setRetry((value) => value + 1)}>
            Retry evidence
          </button>
        </Notice>
      ) : loading ? (
        <p role="status" className="loading">
          Loading evidence…
        </p>
      ) : balances.length === 0 ? (
        <p className="muted evidence-empty">
          {offset
            ? "No more balance observations."
            : "No dated balance yet. Import a statement with balance evidence, or confirm the actual balance."}
        </p>
      ) : (
        <ul className="balance-list">
          {balances.map((balance) => (
            <li key={balance.id}>
              <div>
                <strong className="amount">
                  {money(balance.amount, balance.currency_code)}
                </strong>
                <span className="cell-subtitle">
                  {calendarDate(balance.as_of_date)} ·{" "}
                  {balance.status === "user_confirmed"
                    ? "Confirmed by you"
                    : "Revolut running balance"}
                </span>
                {balance.source_timestamp && (
                  <span className="cell-subtitle">
                    {balance.source_timestamp.slice(11)} · source local time
                  </span>
                )}
                {balance.effective_at && (
                  <span className="cell-subtitle">Checkpoint: {balance.effective_at} (entered at UTC{balance.effective_offset})</span>
                )}
                {balance.note && <p className="balance-note">{balance.note}</p>}
                {report?.balance_status === "conflicted" && report.observation_ids.includes(balance.id) && (
                  <button disabled={resolving || !reason.trim()} onClick={async () => {
                    setResolving(true); setError("");
                    try {
                      await api.resolveBalance(account.id, balance.id, reason.trim());
                      setReason(""); onResolved();
                    } catch (failure) { setError(errorMessage(failure)); }
                    finally { setResolving(false); }
                  }}>Use this observation</button>
                )}
              </div>
              <span
                className={`evidence-dot ${balance.source === "manual" ? "confirmed" : ""}`}
                aria-hidden="true"
              />
            </li>
          ))}
        </ul>
      )}
      <div className="pagination">
        <span className="muted">Evidence stays on record</span>
        <div>
          <button
            disabled={loading || offset === 0}
            onClick={() => setOffset((value) => Math.max(0, value - 10))}
          >
            Newer
          </button>
          <button
            disabled={loading || balances.length < 10}
            onClick={() => setOffset((value) => value + 10)}
          >
            Older
          </button>
        </div>
      </div>
      {report?.balance_status === "conflicted" && balances.some((item) => report.observation_ids.includes(item.id)) && (
        <label className="form-note">Why are you selecting this observation?
          <input value={reason} maxLength={500} onChange={(event) => setReason(event.target.value)} />
        </label>
      )}
    </section>
  );
}

export function ImportHistory({ account, revision }: { account: Account; revision: number }) {
  const [batches, setBatches] = useState<ImportHistoryBatch[]>([]);
  const [error, setError] = useState("");
  const [reason, setReason] = useState("");
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    let active = true;
    api.importHistory(account.id).then((value) => { if (active) setBatches(value); })
      .catch((failure) => { if (active) setError(errorMessage(failure)); });
    return () => { active = false; };
  }, [account.id, revision, retry]);
  async function revise(rowId: string, action: "keep_separate" | "skip", target?: string) {
    if (!reason.trim()) return;
    try {
      await api.reviewImportRow(rowId, action, reason.trim(), target);
      setReason(""); setRetry((value) => value + 1);
    } catch (failure) { setError(errorMessage(failure)); }
  }
  return <section className="panel" aria-labelledby="import-history-title">
    <div className="section-heading"><div><span className="eyebrow">SOURCE HISTORY</span>
      <h2 id="import-history-title">Import history</h2></div></div>
    <p className="muted compact">Review prior batches and revise row decisions. Each change keeps an audit record.</p>
    {error && <Notice error>{error} <button onClick={() => { setError(""); setRetry((value) => value + 1); }}>Retry history</button></Notice>}
    {batches.length === 0 ? <p className="muted">No statement imports yet.</p> : batches.map((batch) =>
      <details key={batch.id} className="report-details"><summary>{batch.parser_id} · {batch.status} · {calendarDate(batch.created_at)}</summary>
        {batch.warnings.map((warning) => <p className="muted" key={warning}>{warning}</p>)}
        {batch.rows.map((row) => <div key={row.id} className="evidence-details">
          <strong>Row {row.row_number}: {row.description || "No description"}</strong>
          <span>{row.amount ?? "Invalid amount"} · {row.decision ?? row.classification}</span>
          {batch.status === "committed" && row.classification !== "invalid" && row.classification !== "excluded" && <>
            <button onClick={() => void revise(row.id, "keep_separate")}>Keep as separate movement</button>
            <button onClick={() => void revise(row.id, "skip")}>Skip this row</button>
            {row.candidates.filter((item) => item.id !== row.transaction_id).map((candidate) =>
              <button key={candidate.id} disabled={!reason.trim()} onClick={() => void api.reviewImportRow(row.id, "link_duplicate", reason.trim(), candidate.id)
                .then(() => setRetry((value) => value + 1)).catch((failure) => setError(errorMessage(failure)))}>
                Link to {candidate.description || candidate.id.slice(0, 8)}
              </button>)}
            {row.posted_at && <TimeCorrection rowId={row.id} postedAt={row.corrected_posted_at ?? row.posted_at}
              offset={row.corrected_offset ?? ""} onSaved={() => setRetry((value) => value + 1)} />}
          </>}
          <AuditTrail rowId={row.id} />
        </div>)}
      </details>)}
    <label className="form-note">Reason for a review change
      <input value={reason} maxLength={500} onChange={(event) => setReason(event.target.value)} />
    </label>
  </section>;
}

function TimeCorrection({ rowId, postedAt, offset, onSaved }: {
  rowId: string; postedAt: string; offset: string; onSaved: () => void;
}) {
  const [time, setTime] = useState(postedAt.slice(0, 16));
  const [utcOffset, setUtcOffset] = useState(offset);
  const [reason, setReason] = useState("");
  const [error, setError] = useState("");
  return <details><summary>Correct ambiguous time</summary>
    <label>Corrected local date and time<input type="datetime-local" step="60" value={time}
      onChange={(event) => setTime(event.target.value)} /></label>
    <label>UTC offset<input value={utcOffset} pattern="[+-][0-9]{2}:[0-9]{2}"
      onChange={(event) => setUtcOffset(event.target.value)} /></label>
    <label>Reason<input value={reason} maxLength={500} onChange={(event) => setReason(event.target.value)} /></label>
    <button disabled={!reason.trim() || !time || !/^[+-][0-9]{2}:[0-9]{2}$/.test(utcOffset)} onClick={async () => {
      setError("");
      try { await api.correctImportTime(rowId, `${time}:00${utcOffset}`, reason.trim()); onSaved(); }
      catch (failure) { setError(errorMessage(failure)); }
    }}>Save time correction</button>
    {error && <Notice error>{error}</Notice>}
  </details>;
}

function AuditTrail({ rowId }: { rowId: string }) {
  const [history, setHistory] = useState<{ id: string; action: string; reason: string; created_at: string }[]>([]);
  useEffect(() => { api.importRowDecisionHistory(rowId).then(setHistory).catch(() => setHistory([])); }, [rowId]);
  return history.length ? <details><summary>Decision changes ({history.length})</summary>
    {history.map((item) => <p key={item.id}>{item.action} · {item.reason} · {new Date(item.created_at).toLocaleString()}</p>)}
  </details> : null;
}
