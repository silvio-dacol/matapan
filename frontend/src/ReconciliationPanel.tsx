import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { api, checkpointTime, errorMessage, localOffset, money } from "./api";
import type { Account, Projection, Reconciliation } from "./api";
import { Notice } from "./Notice";

function PeriodReview({ period, disabled, onChanged, onBusy }: {
  period: Reconciliation; disabled: boolean; onChanged: () => void; onBusy: (busy: boolean) => void;
}) {
  const [approved, setApproved] = useState(false);
  const [reason, setReason] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const canAccept = period.state !== "blocked" && period.state !== "superseded";
  const hasGhost = period.history.some((item) => item.state === "active");
  async function decide(reverse: boolean) {
    setBusy(true); onBusy(true); setError("");
    try {
      await api.adjustment(period.id, period.signature, reason.trim(), reverse);
      onChanged();
    } catch (failure) {
      setError(errorMessage(failure));
      setApproved(false);
    } finally {
      setBusy(false); onBusy(false);
    }
  }
  return <article className="reconciliation-period">
    <h3>{checkpointTime(period.opening)} → {checkpointTime(period.closing)}</h3>
    <p><strong>{period.state.replaceAll("_", " ")}</strong> · History coverage unverified</p>
    <dl className="reconciliation-totals">
      <div><dt>Opening checkpoint</dt><dd>{money(period.opening.amount, period.currency_code)}</dd></div>
      <div><dt>Accepted real movements ({period.movement_count})</dt><dd>{money(period.real_total, period.currency_code)}</dd></div>
      <div><dt>Approved ghost</dt><dd>{money(period.active_adjustment, period.currency_code)}</dd></div>
      <div><dt>Expected closing balance</dt><dd>{money(period.expected_close, period.currency_code)}</dd></div>
      <div><dt>Confirmed closing checkpoint</dt><dd>{money(period.closing.amount, period.currency_code)}</dd></div>
      <div><dt>Remaining difference</dt><dd>{money(period.difference, period.currency_code)}</dd></div>
      <div><dt>Proposed ghost amount</dt><dd>{money(period.required_adjustment, period.currency_code)}</dd></div>
    </dl>
    {period.state === "needs_review" && <Notice>New evidence changed this reconciliation. Review and confirm the proposed ghost amount; the previous approval is preserved.</Notice>}
    <ul className="coverage-diagnostics">{period.coverage_diagnostics.map((item) =>
      <li key={item.code}><strong>{item.message}</strong><span>{item.action}</span></li>)}</ul>
    <ul className="muted">{period.issues.map((issue) => <li key={issue}>{issue}</li>)}</ul>
    <details>
      <summary>Import evidence and row decisions</summary>
      <p>Timestamp convention: {period.evidence.statement_timezone || "Not set"} ({period.evidence.statement_timezone_source === "account" ? "account override" : period.evidence.statement_timezone_source === "matapan" ? "Matapan default" : "required"}). Pending/non-booked rows: {period.evidence.pending_rows}. Skipped rows: {period.evidence.excluded_rows}.</p>
      <p>Observed transaction dates show where rows exist. The CSV has no declared statement period, so they cannot establish full coverage.</p>
      {period.evidence.imports.length === 0 && <p>No imported source rows in this interval.</p>}
      {period.evidence.imports.map((batch) => <div key={batch.id} className="evidence-details">
        <code>{batch.id}</code>
        <p>Observed dates: {batch.observed_start} → {batch.observed_end}. These do not prove full statement coverage.</p>
        <ul>{batch.decisions.map((decision) => <li key={decision.row_id}><code>{decision.row_id}</code>: {decision.action}{decision.transaction_id && ` → ${decision.transaction_id}`}</li>)}</ul>
        <ul>{batch.warnings.map((warning) => <li key={warning}>{warning}</li>)}</ul>
      </div>)}
    </details>
    <p className="muted">Investigate the difference or import missing statements before accepting a ghost. Ghosts compensate for unexplained gaps and are excluded from income and expenses.</p>
    {(canAccept || hasGhost) && <fieldset disabled={disabled || busy} className="adjustment-review">
      <label>Reason for adjustment decision
        <input value={reason} maxLength={500} onChange={(event) => setReason(event.target.value)} placeholder="e.g. Missing statement interval" />
      </label>
      <label className="check-label">
        <input type="checkbox" checked={approved} onChange={(event) => setApproved(event.target.checked)} />
        I reviewed these checkpoints, movements, and the proposed ghost decision.
      </label>
      <div className="button-row">
        {canAccept && <button className="primary" disabled={!approved || !reason.trim()} onClick={() => void decide(false)}>
          {busy ? "Saving…" : "Confirm proposed ghost amount"}
        </button>}
        {hasGhost && <button disabled={!approved || !reason.trim()} onClick={() => void decide(true)}>Retire approved ghost</button>}
      </div>
    </fieldset>}
    {error && <Notice error>{error} <button onClick={onChanged}>Refresh proposal</button></Notice>}
    {period.history.length > 0 && <details>
      <summary>Adjustment audit history ({period.history.length})</summary>
      <ul>{period.history.map((item) => <li key={item.id}>{money(item.amount, period.currency_code)} · {item.state} · {item.reason} · {item.approved_at} · {item.actor}</li>)}</ul>
    </details>}
  </article>;
}

export default function ReconciliationPanel({ account, revision, asOf, disabled, onChanged, onBusy }: {
  account: Account; revision: number; asOf: string; disabled: boolean;
  onChanged: () => void; onBusy: (busy: boolean) => void;
}) {
  const [periods, setPeriods] = useState<Reconciliation[]>([]);
  const [zone, setZone] = useState(account.statement_timezone || "");
  const [savedZone, setSavedZone] = useState(account.statement_timezone);
  const [defaultZone, setDefaultZone] = useState("");
  const [savedDefaultZone, setSavedDefaultZone] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [retry, setRetry] = useState(0);
  const [at, setAt] = useState("");
  const [projection, setProjection] = useState<Projection | null>(null);
  const [projectionError, setProjectionError] = useState("");
  useEffect(() => {
    let active = true;
    setLoading(true); setError(""); setPeriods([]);
    Promise.all([api.reconciliations(account.id), api.matapanTimezone()])
      .then(([result, settings]) => {
        if (active) {
          setPeriods(result);
          setDefaultZone(settings.default_statement_timezone || "");
          setSavedDefaultZone(settings.default_statement_timezone);
        }
      })
      .catch((failure) => { if (active) setError(errorMessage(failure)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [account.id, revision, retry]);
  useEffect(() => {
    if (!at) return;
    let active = true;
    setProjection(null); setProjectionError("");
    api.projection(account.id, at).then((result) => { if (active) setProjection(result); })
      .catch((failure) => { if (active) setProjectionError(errorMessage(failure)); });
    return () => { active = false; };
  }, [account.id, at, revision, retry]);
  async function saveZone(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setBusy(true); onBusy(true); setError("");
    try {
      const updated = await api.statementTimezone(account.id, zone.trim() || null);
      setSavedZone(updated.statement_timezone); onChanged();
    } catch (failure) { setError(errorMessage(failure)); }
    finally { setBusy(false); onBusy(false); }
  }
  async function saveDefaultZone(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setBusy(true); onBusy(true); setError("");
    try {
      const updated = await api.setMatapanTimezone(defaultZone.trim());
      setSavedDefaultZone(updated.default_statement_timezone); onChanged();
    } catch (failure) { setError(errorMessage(failure)); }
    finally { setBusy(false); onBusy(false); }
  }
  function calculate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const local = String(data.get("projection-time"));
    setAt(`${local}${local.length === 16 ? ":00" : ""}${String(data.get("projection-offset"))}`);
    setRetry((value) => value + 1);
  }
  return <section className="panel reconciliation-panel" aria-labelledby="reconciliation-title">
    <div className="section-heading"><div><span className="eyebrow">CHECKPOINTS AND GAPS</span><h2 id="reconciliation-title">Balance reconciliation</h2></div>
      <button disabled={disabled || busy} onClick={() => setRetry((value) => value + 1)}>Refresh reconciliation</button>
    </div>
    <p className="muted">Your first timed confirmation establishes the reference balance. Before then, a calculated balance is unavailable rather than zero. Date-only observations keep their original date-only meaning and remain in the evidence report; Matapan does not assign them invented times. Later checkpoints reveal unexplained differences. New imports automatically recalculate proposals; approval is always yours.</p>
    <form onSubmit={saveDefaultZone}>
      <fieldset disabled={disabled || busy} className="form-grid">
        <label>Matapan default timezone
          <input value={defaultZone} onChange={(event) => setDefaultZone(event.target.value)} required maxLength={100} placeholder="Europe/Zurich" />
        </label>
        <div className="button-row"><button type="submit">Save Matapan timezone</button></div>
        <p className="form-note wide">Used for imported local timestamps when an account has no override. Choose an IANA timezone. Matapan does not infer it from file metadata. Changing it recalculates proposals and may require review. Saved: {savedDefaultZone || "Not set"}.</p>
      </fieldset>
    </form>
    <form onSubmit={saveZone}>
      <fieldset disabled={disabled || busy} className="form-grid">
        <label>Account-specific timezone override
          <input value={zone} onChange={(event) => setZone(event.target.value)} maxLength={100} placeholder={savedDefaultZone || "Use Matapan default"} />
        </label>
        <div className="button-row"><button type="submit">{zone.trim() ? "Save account override" : "Use Matapan default"}</button></div>
        <p className="form-note wide">An override takes precedence over Matapan's default for this account. Existing override: {savedZone || "None"}. Changing it recalculates proposals and may require review.</p>
      </fieldset>
    </form>
    <form onSubmit={calculate} className="projection-form">
      <fieldset disabled={disabled || busy} className="form-grid">
        <label>Calculate balance at
          <input name="projection-time" type="datetime-local" step="1" defaultValue={`${asOf}T23:59:59`} required />
        </label>
        <label>Calculation UTC offset
          <input name="projection-offset" defaultValue={localOffset()} pattern="[+-][0-9]{2}:[0-9]{2}" required />
        </label>
        <div className="button-row wide"><button type="submit">Calculate from checkpoint</button></div>
      </fieldset>
    </form>
    {at && <div className="projection-result" aria-live="polite">
      <h3>Calculated account balance</h3>
      {projectionError ? <Notice error>{projectionError}</Notice> : !projection ? <p>Calculating…</p> : <>
        <p className="amount">{money(projection.amount, account.currency_code)} · {projection.status}</p>
        <p>At {projection.at}</p>
        {projection.checkpoint && <p>Reference: {money(projection.checkpoint.amount, account.currency_code)} at {checkpointTime(projection.checkpoint)}. Subsequent movements: {projection.movement_count} ({money(projection.real_total, account.currency_code)}).</p>}
        <ul className="muted">{projection.issues.map((issue) => <li key={issue}>{issue}</li>)}</ul>
      </>}
    </div>}
    {error && <Notice error>{error} <button onClick={() => setRetry((value) => value + 1)}>Retry reconciliation</button></Notice>}
    {loading ? <p role="status">Loading checkpoint intervals…</p> : periods.length === 0 ? <p>Confirm two timed balances to compare an interval. One timed checkpoint is enough to calculate a later balance.</p> : periods.map((period) =>
      <PeriodReview key={`${period.id}-${period.signature}`} period={period} disabled={disabled || busy} onBusy={onBusy} onChanged={onChanged} />)}
  </section>;
}
