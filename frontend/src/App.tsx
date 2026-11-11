import { useCallback, useEffect, useState } from "react";
import { api, calendarDate, errorMessage, money, today } from "./api";
import type { Account, AccountReport } from "./api";
import ImportPanel from "./ImportPanel";
import { AccountForm, BalanceForm, ManualMovementForm } from "./AccountForms";
import { Activity, BalanceHistory, ImportHistory } from "./AccountRecords";
import { Notice } from "./Notice";
import ReconciliationPanel from "./ReconciliationPanel";

function Mark({ small = false }: { small?: boolean }) {
  return (
    <span className={`brand-mark ${small ? "small" : ""}`} aria-hidden="true">
      m<span>↗</span>
    </span>
  );
}

function Workspace({
  account,
  onLock,
}: {
  account: Account;
  onLock: (value: boolean) => void;
}) {
  const [asOf, setAsOf] = useState(today);
  const [report, setReport] = useState<AccountReport | null>(null);
  const [reportError, setReportError] = useState("");
  const [loading, setLoading] = useState(true);
  const [revision, setRevision] = useState(0);
  const [panel, setPanel] = useState<"import" | "balance" | "movement" | null>(null);
  const [message, setMessage] = useState("");
  const [reconBusy, setReconBusy] = useState(false);
  const refresh = useCallback(() => setRevision((value) => value + 1), []);
  const eligible =
    account.revolut_product !== null &&
    (!account.institution || /revolut/i.test(account.institution));

  useEffect(() => {
    onLock(panel !== null || reconBusy);
  }, [panel, reconBusy, onLock]);
  useEffect(() => {
    let active = true;
    setLoading(true);
    setReportError("");
    setReport(null);
    api
      .report(account.id, asOf)
      .then((result) => {
        if (active) setReport(result);
      })
      .catch((failure) => {
        if (active) setReportError(errorMessage(failure));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [account.id, asOf, revision]);

  const currentReport = report?.as_of_date === asOf ? report : null;
  const badge =
    currentReport?.balance_status === "user_confirmed"
      ? "Confirmed by you"
      : currentReport?.balance_status === "source_reported"
        ? "Source reported"
        : currentReport?.balance_status === "conflicted"
          ? "Conflicting evidence"
          : "Balance needed";

  return (
    <>
      <header className="workspace-header">
        <div>
          <span className="eyebrow">ACCOUNT WORKSPACE</span>
          <h1>{account.name}</h1>
          <p>
            {account.institution || "Manual account"}{" "}
            <span aria-hidden="true">/</span>{" "}
            {account.revolut_product || "Cash"}{" "}
            <span aria-hidden="true">/</span> {account.currency_code}
          </p>
        </div>
        <div className="button-row">
          <button
            disabled={panel !== null || reconBusy}
            onClick={() => {
              setMessage("");
              setPanel("balance");
            }}
          >
            Confirm balance
          </button>
          <button disabled={panel !== null || reconBusy} onClick={() => setPanel("movement")}>
            Add movement
          </button>
          <button
            className="primary"
            disabled={panel !== null || reconBusy || !eligible}
            onClick={() => {
              setMessage("");
              setPanel("import");
            }}
          >
            <span aria-hidden="true">↥</span> Import statement
          </button>
        </div>
      </header>
      {message && <Notice>{message}</Notice>}
      {panel === "balance" && (
        <BalanceForm
          account={account}
          asOf={asOf}
          onCancel={() => setPanel(null)}
          onSaved={(date) => {
            setPanel(null);
            setAsOf(date);
            refresh();
            setMessage(
              "Your dated balance confirmation was saved. Existing evidence is preserved; check the report below for any conflicts.",
            );
          }}
        />
      )}
      {panel === "movement" && <ManualMovementForm account={account} onCancel={() => setPanel(null)}
        onSaved={() => { setPanel(null); refresh(); setMessage("Manual movement saved with its source and reason."); }} />}
      {panel === "import" && (
        <ImportPanel
          account={account}
          onClose={() => setPanel(null)}
          onCommitted={(result) => {
            refresh();
            setMessage(result);
          }}
        />
      )}
      <div className="overview-grid">
        <section className="balance-card" aria-labelledby="cash-title">
          <div className="balance-top">
            <span className="eyebrow" id="cash-title">
              ACCOUNT CASH
            </span>
            <span
              className={`status-badge ${currentReport?.balance_status === "conflicted" ? "conflict" : ""}`}
            >
              {loading ? "Loading…" : reportError ? "Could not load" : badge}
            </span>
          </div>
          <div className="balance-value" aria-live="polite">
            {loading ? (
              "…"
            ) : currentReport?.amount != null ? (
              <>
                {money(currentReport.amount, "").trim()}
                <span>{account.currency_code}</span>
              </>
            ) : (
              "Not available"
            )}
          </div>
          <p className="balance-date">
            {currentReport?.balance_as_of_date
              ? `Observed ${calendarDate(currentReport.balance_as_of_date)}`
              : "A dated balance gives this account a value."}
            {currentReport?.source_timestamp && (
              <span>
                At {currentReport.source_timestamp.slice(11)} · source local
                time
              </span>
            )}
            {currentReport?.balance_effective_at && <span>Checkpoint: {currentReport.balance_effective_at}</span>}
          </p>
          <div className="balance-footer">
            <label>
              Report as of
              <input
                aria-label="Report as of"
                type="date"
                value={asOf}
                onChange={(event) => {
                  if (event.target.value) setAsOf(event.target.value);
                }}
              />
            </label>
            <span>
              Account cash only
              <br />
              Partial financial picture
            </span>
          </div>
        </section>
        <section
          className="panel quality-panel"
          aria-labelledby="quality-title"
        >
          <span className="eyebrow">UNDERSTAND THIS VALUE</span>
          <h2 id="quality-title">A balance with context</h2>
          <div className="quality-item">
            <span className="quality-icon" aria-hidden="true">
              ◎
            </span>
            <div>
              <h3>
                {loading
                  ? "Checking dated evidence"
                  : reportError
                    ? "Evidence could not be loaded"
                    : currentReport?.carried_forward
                      ? "An earlier observation"
                      : currentReport?.balance_status === "conflicted"
                        ? "This balance needs review"
                        : currentReport?.amount != null
                          ? "Dated evidence"
                          : "A balance is still needed"}
              </h3>
              <p>
                {loading
                  ? "Looking for balance evidence on or before the report date."
                  : reportError
                    ? "Retry the report to read your saved evidence. A connection problem does not mean the account balance is missing."
                    : currentReport?.carried_forward
                      ? "Carried forward from the observation date. It has not been confirmed for the report date."
                      : currentReport?.balance_status === "user_confirmed"
                        ? "You confirmed this amount for its stated date."
                        : currentReport?.balance_status === "source_reported"
                          ? "A running balance from Revolut, rather than a verified statement-end or end-of-day balance."
                          : currentReport?.balance_status === "conflicted"
                            ? "Different amounts exist for the same date. They are preserved, and no amount is selected."
                            : "Confirm the actual balance or import a statement that supplies balance evidence."}
              </p>
            </div>
          </div>
          <div className="quality-item">
            <span className="quality-icon amber" aria-hidden="true">
              ↔
            </span>
            <div>
              <h3>History coverage unverified</h3>
              <p>
                Imported activity may be incomplete. Confirming a balance does
                not fill transaction gaps or reconcile the ledger.
              </p>
            </div>
          </div>
          {reportError && (
            <Notice error>
              {reportError} <button onClick={refresh}>Retry report</button>
            </Notice>
          )}
          {currentReport && currentReport.issues.length > 0 && (
            <details className="report-details">
              <summary>Report notes ({currentReport.issues.length})</summary>
              <ul>
                {currentReport.issues.map((issue) => (
                  <li key={issue}>{issue}</li>
                ))}
              </ul>
            </details>
          )}
          {currentReport?.resolution && (
            <p className="form-note">Balance selection recorded: {currentReport.resolution.reason}</p>
          )}
        </section>
      </div>
      {!eligible && (
        <div className="subtle-note">
          This account supports dated balance confirmations. CSV import is
          available for Revolut Current and Savings accounts.
        </div>
      )}
      <Activity account={account} revision={revision} />
      <ReconciliationPanel account={account} revision={revision} asOf={asOf} disabled={panel !== null || reconBusy} onChanged={refresh} onBusy={setReconBusy} />
      <ImportHistory account={account} revision={revision} />
      <BalanceHistory account={account} revision={revision} report={currentReport} onResolved={refresh} />
    </>
  );
}

export default function App() {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [selected, setSelected] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [connected, setConnected] = useState(false);
  const [creating, setCreating] = useState(false);
  const [locked, setLocked] = useState(false);
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    let active = true;
    setLoading(true);
    setError("");
    Promise.all([api.accounts(), api.health()])
      .then(([result]) => {
        if (active) {
          setAccounts(result);
          setSelected((previous) =>
            result.some((account) => account.id === previous)
              ? previous
              : result[0]?.id || "",
          );
          setConnected(true);
        }
      })
      .catch((failure) => {
        if (active) {
          setError(errorMessage(failure));
          setConnected(false);
        }
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [retry]);
  const account = accounts.find((item) => item.id === selected);
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a href="#main" className="brand">
          <Mark />
          <span>
            matapan<span className="brand-caption">Know where you stand.</span>
          </span>
        </a>
        <div className="sidebar-section-heading">
          <span className="eyebrow">ACCOUNTS</span>
          <span className="account-count">{accounts.length}</span>
        </div>
        <nav aria-label="Accounts" className="account-nav">
          {accounts.map((item) => (
            <button
              key={item.id}
              className={`account-button ${selected === item.id ? "selected" : ""}`}
              disabled={locked || creating}
              aria-current={selected === item.id ? "page" : undefined}
              onClick={() => setSelected(item.id)}
            >
              <span className="account-avatar" aria-hidden="true">
                {(item.institution || item.name).slice(0, 1)}
              </span>
              <span className="account-label">
                <strong>{item.name}</strong>
                <span>
                  {item.revolut_product || item.institution || "Cash"} ·{" "}
                  {item.currency_code}
                </span>
              </span>
              <span className="nav-arrow" aria-hidden="true">
                ›
              </span>
            </button>
          ))}
        </nav>
        {!loading && !error && accounts.length === 0 && (
          <p className="sidebar-empty">Your accounts will appear here.</p>
        )}
        <button
          className="add-account"
          disabled={locked || creating || loading || !connected}
          onClick={() => setCreating(true)}
        >
          <span aria-hidden="true">+</span> Add account
        </button>
        <div className="sidebar-bottom">
          <div className="local-label">
            <span className={`connection-dot ${connected ? "online" : ""}`} />
            {connected
              ? "Local API connected"
              : loading
                ? "Connecting…"
                : "API unavailable"}
          </div>
          <p>
            Your records stay in your local
            <br />
            Matapan database.
          </p>
          <span className="version-label">FIRST CHAPTER · ACCOUNT CASH</span>
        </div>
      </aside>
      <main id="main" className="main-content">
        <div className="top-bar">
          <span>Your financial workspace</span>
          <span className="local-tag">
            <span aria-hidden="true">⌂</span> Local workspace
          </span>
        </div>
        {loading ? (
          <div className="welcome">
            <p role="status">Connecting to your accounts…</p>
          </div>
        ) : error ? (
          <div className="welcome">
            <span className="eyebrow">LET’S GET CONNECTED</span>
            <h1>Your workspace is waiting.</h1>
            <Notice error>{error}</Notice>
            <p className="muted">
              Start the backend on port 8000, then try again.
            </p>
            <button
              className="primary"
              onClick={() => setRetry((value) => value + 1)}
            >
              Retry connection
            </button>
          </div>
        ) : creating ? (
          <>
            <header className="workspace-header">
              <div>
                <span className="eyebrow">A CLEARER FINANCIAL PICTURE</span>
                <h1>One account at a time.</h1>
              </div>
            </header>
            <AccountForm
              onCancel={() => setCreating(false)}
              onCreated={(created) => {
                setAccounts((previous) => [...previous, created]);
                setSelected(created.id);
                setCreating(false);
              }}
            />
          </>
        ) : account ? (
          <Workspace key={account.id} account={account} onLock={setLocked} />
        ) : (
          <section className="welcome">
            <Mark />
            <span className="eyebrow">WELCOME TO MATAPAN</span>
            <h1>
              A clear view starts
              <br />
              with one account.
            </h1>
            <p>
              Add an account, review a statement, and see the evidence behind
              your balance. If your history is incomplete, you can confirm the
              actual balance for a specific date.
            </p>
            <button className="primary" onClick={() => setCreating(true)}>
              Add your first account <span aria-hidden="true">↗</span>
            </button>
            <div className="welcome-steps">
              <span>
                <b>01</b> Add your account
              </span>
              <span>
                <b>02</b> Import & review
              </span>
              <span>
                <b>03</b> Know your balance
              </span>
            </div>
          </section>
        )}
        <footer className="page-footer">
          <span>Matapan</span>
          <span>Every balance has a date. Every figure has a source.</span>
        </footer>
      </main>
    </div>
  );
}
