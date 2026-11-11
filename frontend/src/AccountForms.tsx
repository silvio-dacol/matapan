import { useState } from "react";
import type { FormEvent } from "react";
import { api, errorMessage, localOffset } from "./api";
import type { Account } from "./api";
import { Notice } from "./Notice";

export function AccountForm({
  onCreated,
  onCancel,
}: {
  onCreated: (account: Account) => void;
  onCancel: () => void;
}) {
  const [provider, setProvider] = useState("Revolut");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    setBusy(true);
    setError("");
    try {
      const account = await api.createAccount({
        name: String(data.get("name")).trim(),
        institution:
          provider === "Revolut"
            ? "Revolut"
            : String(data.get("institution")).trim() || null,
        currency_code: String(data.get("currency")).trim().toUpperCase(),
        revolut_product:
          provider === "Revolut"
            ? (data.get("product") as "Current" | "Savings")
            : null,
      });
      onCreated(account);
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel form-panel" aria-labelledby="create-title">
      <div className="section-heading">
        <div>
          <span className="eyebrow">YOUR ACCOUNTS</span>
          <h2 id="create-title">Add an account</h2>
        </div>
      </div>
      <p className="muted">
        Give each currency and Revolut product its own account. You can confirm
        a balance even without importing a statement.
      </p>
      <form onSubmit={submit}>
        <fieldset disabled={busy} className="form-grid">
          <label className="wide">
            Account name
            <input
              name="name"
              required
              maxLength={120}
              placeholder="e.g. Everyday EUR"
              autoFocus
            />
          </label>
          <label>
            Institution
            <select
              value={provider}
              onChange={(event) => setProvider(event.target.value)}
            >
              <option>Revolut</option>
              <option value="Other">Other institution</option>
            </select>
          </label>
          <label>
            Currency
            <input
              name="currency"
              defaultValue="EUR"
              required
              pattern="[A-Za-z]{3}"
              maxLength={3}
              className="uppercase"
            />
          </label>
          {provider === "Revolut" ? (
            <label className="wide">
              Revolut product
              <select name="product">
                <option value="Current">Current account</option>
                <option value="Savings">Savings</option>
              </select>
            </label>
          ) : (
            <label className="wide">
              Institution name (optional)
              <input
                name="institution"
                maxLength={120}
                placeholder="Your bank"
              />
            </label>
          )}
          {provider === "Other" && (
            <p className="form-note wide">
              Statement import currently supports Revolut only. Other accounts
              support manual dated balances.
            </p>
          )}
          <div className="button-row wide">
            <button className="primary" type="submit">
              {busy ? "Creating…" : "Create account"}
            </button>
            <button type="button" onClick={onCancel}>
              Cancel
            </button>
          </div>
        </fieldset>
        {error && <Notice error>{error}</Notice>}
      </form>
    </section>
  );
}

export function BalanceForm({
  account,
  asOf,
  onSaved,
  onCancel,
}: {
  account: Account;
  asOf: string;
  onSaved: (date: string) => void;
  onCancel: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [timed, setTimed] = useState(false);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const date = String(data.get("date"));
    setBusy(true);
    setError("");
    try {
      await api.confirmBalance(account.id, {
        currency_code: account.currency_code,
        as_of_date: date,
        amount: String(data.get("amount")).trim(),
        note: String(data.get("note")).trim() || null,
        ...(timed ? { effective_at: `${date}T${String(data.get("time"))}${String(data.get("time")).length === 5 ? ":00" : ""}${String(data.get("offset"))}` } : {}),
      });
      onSaved(date);
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel form-panel" aria-labelledby="balance-title">
      <div className="section-heading">
        <div>
          <span className="eyebrow">DATED CONFIRMATION</span>
          <h2 id="balance-title">Confirm the actual balance</h2>
        </div>
        <span className="tag">{account.currency_code}</span>
      </div>
      <p className="muted">
        Check your bank and enter the balance for the date below. This records
        what you confirmed, even if you have only imported part of your history.
      </p>
      <form onSubmit={submit}>
        <fieldset disabled={busy} className="form-grid">
          <label>
            Actual balance ({account.currency_code})
            <input
              name="amount"
              type="text"
              inputMode="decimal"
              required
              pattern="-?(0|[1-9][0-9]{0,19})(\.[0-9]{1,8})?"
              placeholder="1234.50"
              aria-describedby="money-help"
              autoFocus
            />
          </label>
          <label>
            Balance date
            <input name="date" type="date" defaultValue={asOf} required />
          </label>
          <p id="money-help" className="form-note wide">
            Use a decimal point, without thousands separators. Negative balances
            are allowed.
          </p>
          <label className="check-label wide">
            <input type="checkbox" checked={timed} onChange={(event) => setTimed(event.target.checked)} />
            Use this balance as a timed reference checkpoint
          </label>
          {timed && <>
            <label>Checkpoint time
              <input name="time" type="time" step="1" defaultValue="12:00:00" required />
            </label>
            <label>Checkpoint UTC offset
              <input name="offset" defaultValue={localOffset()} required pattern="[+-][0-9]{2}:[0-9]{2}" placeholder="+02:00" />
            </label>
            <p className="form-note wide">Confirm the booked cash balance including all booked movements up to and including this instant, excluding pending movements. Check the UTC offset for that date, including daylight saving.</p>
          </>}
          <label className="wide">
            Note (optional)
            <textarea
              name="note"
              maxLength={500}
              rows={2}
              placeholder="e.g. Checked in the bank app"
            />
          </label>
          <div className="subtle-note wide">
            A confirmation preserves existing evidence and does not fill missing
            transactions. Timed checkpoints let you review differences and approve
            ghost adjustments below. Conflicting amounts at the same checkpoint
            require investigation.
          </div>
          <label className="check-label wide">
            <input type="checkbox" required />I confirm this is the account
            balance for the stated date.
          </label>
          <div className="button-row wide">
            <button type="submit" className="primary">
              {busy ? "Saving…" : "Save confirmed balance"}
            </button>
            <button type="button" onClick={onCancel}>
              Cancel
            </button>
          </div>
        </fieldset>
        {error && <Notice error>{error}</Notice>}
      </form>
    </section>
  );
}

export function ManualMovementForm({ account, onCancel, onSaved }: {
  account: Account; onCancel: () => void; onSaved: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    setBusy(true); setError("");
    try {
      await api.createManualMovement(account.id, {
        posted_at: `${String(data.get("time"))}:00${String(data.get("offset"))}`,
        amount: String(data.get("amount")), description: String(data.get("description")).trim(),
        reason: String(data.get("reason")).trim(),
      });
      onSaved();
    } catch (failure) { setError(errorMessage(failure)); }
    finally { setBusy(false); }
  }
  return <section className="panel form-panel" aria-labelledby="movement-title">
    <div className="section-heading"><div><span className="eyebrow">MANUAL ACTIVITY</span>
      <h2 id="movement-title">Add a movement</h2></div></div>
    <p className="muted">This records a movement with manual provenance. It does not change a confirmed balance.</p>
    <form onSubmit={submit}><fieldset disabled={busy} className="form-grid">
      <label>Signed amount ({account.currency_code})<input name="amount" required inputMode="decimal" pattern="-?(0|[1-9][0-9]*)(\\.[0-9]{1,8})?" /></label>
      <label>Date and time<input name="time" type="datetime-local" step="60" required /></label>
      <label>UTC offset<input name="offset" defaultValue={localOffset()} required pattern="[+-][0-9]{2}:[0-9]{2}" /></label>
      <label className="wide">Description<input name="description" required maxLength={500} /></label>
      <label className="wide">Why is this movement being added?<textarea name="reason" required maxLength={500} rows={2} /></label>
      <div className="button-row wide"><button className="primary" type="submit">Save movement</button>
        <button type="button" onClick={onCancel}>Cancel</button></div>
    </fieldset>{error && <Notice error>{error}</Notice>}</form>
  </section>;
}
