"""Checkpoint arithmetic and the approved ghost lifecycle use real API/imports."""

from concurrent.futures import ThreadPoolExecutor
from decimal import Decimal

import pytest
from fastapi.testclient import TestClient

from matapan.main import create_app


def checkpoint(client, account, amount, at):
    response = client.post(
        f"/api/v1/accounts/{account['id']}/cash-balances",
        json={
            "currency_code": account["currency_code"],
            "as_of_date": at[:10],
            "amount": amount,
            "effective_at": at,
        },
    )
    assert response.status_code == 201, response.text
    return response.json()


def zone(client, account, name="UTC"):
    response = client.put(
        f"/api/v1/accounts/{account['id']}/statement-timezone", json={"statement_timezone": name}
    )
    assert response.status_code == 200, response.text


def periods(client, account):
    response = client.get(f"/api/v1/accounts/{account['id']}/reconciliations")
    assert response.status_code == 200, response.text
    return response.json()


def approve(client, period, reverse=False):
    return client.post(
        f"/api/v1/reconciliations/{period['id']}/{'reverse' if reverse else 'accept'}-adjustment",
        json={"signature": period["signature"], "reason": "Synthetic test approval"},
    )


def import_rows(client, account, content, decisions=None):
    preview = client.post(
        "/api/v1/imports/preview",
        data={"account_id": account["id"]},
        files={"file": ("synthetic.csv", content, "text/csv")},
    )
    assert preview.status_code == 200, preview.text
    data = preview.json()
    response = client.post(
        f"/api/v1/imports/{data['id']}/commit",
        json={
            "include_balance": False,
            "decisions": decisions(data) if decisions else [],
        },
    )
    assert response.status_code == 200, response.text
    return response.json()


def project(client, account, at):
    response = client.get(f"/api/v1/accounts/{account['id']}/projection", params={"at": at})
    assert response.status_code == 200, response.text
    return response.json()


def test_first_checkpoint_rebases_without_counting_earlier_movements(
    client, revolut_account, make_csv
):
    account = revolut_account
    zone(client, account)
    import_rows(
        client,
        account,
        make_csv(
            {"Completed Date": "2025-01-01 09:00:00", "Amount": "700"},
            {"Completed Date": "2025-01-01 12:00:00", "Amount": "300"},
            {"Completed Date": "2025-01-01 12:00:01", "Amount": "-50"},
            {"Completed Date": "2025-01-02 09:00:00", "Amount": "20", "Fee": "1"},
        ),
    )
    assert project(client, account, "2025-01-02T09:00:00Z")["amount"] is None
    checkpoint(client, account, "1000", "2025-01-01T12:00:00Z")
    assert periods(client, account) == []
    at_checkpoint = project(client, account, "2025-01-01T12:00:00Z")
    assert at_checkpoint["amount"] == "1000"
    assert at_checkpoint["status"] == "confirmed"
    later = project(client, account, "2025-01-02T09:00:00Z")
    assert Decimal(later["amount"]) == Decimal("969")
    assert later["movement_count"] == 2
    assert later["status"] == "calculated"
    assert project(client, account, "2025-01-01T11:59:59Z")["amount"] is None


def test_late_data_reduces_then_retires_ghost_only_with_approval(
    client,
    revolut_account,
    make_csv,
    database_url,
):
    account = revolut_account
    zone(client, account)
    checkpoint(client, account, "1000", "2025-01-01T00:00:00Z")
    close = checkpoint(client, account, "2800", "2025-02-01T00:00:00Z")
    initial = periods(client, account)[0]
    assert initial["state"] == "unresolved"
    assert Decimal(initial["required_adjustment"]) == 1800
    accepted = approve(client, initial)
    assert accepted.status_code == 200, accepted.text
    accepted = accepted.json()
    original_evidence = accepted["history"][0]["evidence"]
    assert accepted["state"] == "reconciled"
    assert accepted["coverage"] == "unverified"
    assert approve(client, initial).status_code == 409

    import_rows(
        client,
        account,
        make_csv(
            {
                "Completed Date": "2025-01-15 10:00:00",
                "Amount": "1200",
            }
        ),
    )
    revised = periods(client, account)[0]
    assert revised["state"] == "needs_review"
    assert Decimal(revised["active_adjustment"]) == 1800
    assert Decimal(revised["required_adjustment"]) == 600
    assert revised["history"][0]["evidence"] == original_evidence
    assert approve(client, accepted).status_code == 409
    accepted = approve(client, revised).json()
    assert accepted["state"] == "reconciled"
    assert [item["state"] for item in accepted["history"]] == ["superseded", "active"]

    import_rows(
        client,
        account,
        make_csv(
            {
                "Completed Date": "2025-01-20 10:00:00",
                "Amount": "600",
            }
        ),
    )
    zero = periods(client, account)[0]
    assert zero["state"] == "needs_review"
    assert Decimal(zero["required_adjustment"]) == 0
    assert Decimal(zero["active_adjustment"]) == 600
    retired = approve(client, zero).json()
    assert retired["state"] == "reconciled"
    assert Decimal(retired["active_adjustment"]) == 0
    assert len(retired["history"]) == 3
    assert retired["history"][-1]["action"] == "reverse"
    # The closing checkpoint is authoritative. Never add its ghost or preceding movements again.
    assert project(client, account, "2025-02-02T00:00:00Z")["amount"] == "2800"
    report = client.get(f"/api/v1/accounts/{account['id']}/report?as_of=2025-02-01").json()
    assert report["amount"] == "2800"
    assert report["observation_ids"] == [close["id"]]
    assert len(client.get(f"/api/v1/accounts/{account['id']}/transactions").json()) == 2
    with TestClient(create_app(database_url), base_url="http://localhost") as restarted:
        assert periods(restarted, account)[0]["history"] == retired["history"]


def test_voluntary_reversal_preserves_checkpoints(client, account):
    zone(client, account)
    checkpoint(client, account, "100", "2025-01-01T00:00:00Z")
    checkpoint(client, account, "90", "2025-01-02T00:00:00Z")
    accepted = approve(client, periods(client, account)[0]).json()
    assert accepted["active_adjustment"] == "-10"
    reversed_period = approve(client, accepted, reverse=True).json()
    assert reversed_period["state"] == "unresolved"
    assert len(reversed_period["history"]) == 2
    assert project(client, account, "2025-01-02T00:00:00Z")["amount"] == "90"


def test_duplicate_imports_count_canonical_movement_once(client, revolut_account, make_csv):
    account = revolut_account
    zone(client, account)
    checkpoint(client, account, "100", "2025-01-01T00:00:00Z")
    checkpoint(client, account, "90", "2025-02-01T00:00:00Z")
    content = make_csv({})
    import_rows(client, account, content)
    first = periods(client, account)[0]
    import_rows(client, account, content)
    assert periods(client, account)[0]["signature"] == first["signature"]
    import_rows(
        client,
        account,
        content + b"\n",
        lambda preview: [
            {
                "row_id": row["id"],
                "action": "link_duplicate",
                "transaction_id": row["candidate_transactions"][0]["id"],
            }
            for row in preview["rows"]
        ],
    )
    current = periods(client, account)[0]
    assert current["movement_count"] == 1
    assert Decimal(current["required_adjustment"]) == 0
    assert len(current["evidence"]["imports"]) == 2


def test_missing_timezone_blocks_calculation_and_approval(client, revolut_account):
    checkpoint(client, revolut_account, "100", "2025-01-01T00:00:00Z")
    checkpoint(client, revolut_account, "120", "2025-02-01T00:00:00Z")
    period = periods(client, revolut_account)[0]
    assert period["state"] == "blocked"
    assert approve(client, period).status_code == 409
    assert project(client, revolut_account, "2025-02-02T00:00:00Z")["amount"] is None
    response = client.put(
        f"/api/v1/accounts/{revolut_account['id']}/statement-timezone",
        json={"statement_timezone": "Invented/Zone"},
    )
    assert response.status_code == 422


@pytest.mark.parametrize("local", ["2025-03-30 02:30:00", "2025-10-26 02:30:00"])
def test_dst_gap_and_fold_block_automatic_reconciliation(client, revolut_account, make_csv, local):
    zone(client, revolut_account, "Europe/Zurich")
    day = local[:10]
    checkpoint(client, revolut_account, "100", f"{day}T00:00:00Z")
    checkpoint(client, revolut_account, "90", f"{day}T23:00:00Z")
    import_rows(client, revolut_account, make_csv({"Completed Date": local}))
    period = periods(client, revolut_account)[0]
    assert period["state"] == "blocked"
    assert period["required_adjustment"] is None
    assert approve(client, period).status_code == 409


def test_explicit_offset_and_statement_timezone_agree_across_utc_day(
    client, revolut_account, make_csv
):
    zone(client, revolut_account, "Europe/Zurich")
    saved = checkpoint(client, revolut_account, "100", "2025-01-03T00:30:00+01:00")
    assert saved["effective_at"] == "2025-01-02T23:30:00Z"
    assert saved["effective_offset"] == "+01:00"
    import_rows(client, revolut_account, make_csv({"Completed Date": "2025-01-03 01:00:00"}))
    assert Decimal(project(client, revolut_account, "2025-01-03T00:00:00Z")["amount"]) == 90


def test_same_day_checkpoints_are_distinct_but_same_instant_conflicts_block(client, account):
    zone(client, account)
    checkpoint(client, account, "100", "2025-01-01T10:00:00Z")
    checkpoint(client, account, "120", "2025-01-01T12:00:00Z")
    report = client.get(f"/api/v1/accounts/{account['id']}/report?as_of=2025-01-01").json()
    assert report["amount"] == "120"
    assert periods(client, account)[0]["state"] == "unresolved"
    checkpoint(client, account, "121", "2025-01-01T12:00:00Z")
    assert periods(client, account)[0]["state"] == "blocked"
    assert project(client, account, "2025-01-01T12:00:00Z")["amount"] is None


def test_inserting_checkpoint_cannot_double_count_overlapping_ghosts(client, account):
    zone(client, account)
    checkpoint(client, account, "100", "2025-01-01T00:00:00Z")
    checkpoint(client, account, "150", "2025-01-03T00:00:00Z")
    approve(client, periods(client, account)[0])
    checkpoint(client, account, "120", "2025-01-02T00:00:00Z")
    all_periods = periods(client, account)
    old = next(item for item in all_periods if not item["current"])
    assert old["state"] == "superseded"
    assert all(item["state"] == "blocked" for item in all_periods if item["current"])
    assert approve(client, old, reverse=True).status_code == 200
    assert all(
        item["state"] == "unresolved" for item in periods(client, account) if item["current"]
    )


def test_concurrent_approvals_cannot_create_two_active_ghosts(client, account):
    zone(client, account)
    checkpoint(client, account, "100", "2025-01-01T00:00:00Z")
    checkpoint(client, account, "150", "2025-01-02T00:00:00Z")
    proposal = periods(client, account)[0]
    with ThreadPoolExecutor(max_workers=2) as executor:
        responses = list(executor.map(lambda _: approve(client, proposal), range(2)))
    assert sorted(response.status_code for response in responses) == [200, 409]
    assert len(periods(client, account)[0]["history"]) == 1


def test_calculation_preserves_all_decimal_digits(client, revolut_account, make_csv):
    zone(client, revolut_account)
    checkpoint(client, revolut_account, "99999999999999999999.12345678", "2025-01-01T00:00:00Z")
    import_rows(client, revolut_account, make_csv({"Amount": "0.00000001", "Fee": "0"}))
    assert (
        project(client, revolut_account, "2025-01-04T00:00:00Z")["amount"]
        == "99999999999999999999.12345679"
    )


@pytest.mark.parametrize(
    "at", ["2025-01-01T12:00:00", "2025-01-01", 1234, "2025-01-01T12:00:00+15:00"]
)
def test_timed_confirmation_rejects_ambiguous_instants(client, account, at):
    response = client.post(
        f"/api/v1/accounts/{account['id']}/cash-balances",
        json={
            "currency_code": "CHF",
            "as_of_date": "2025-01-01",
            "amount": "100",
            "effective_at": at,
        },
    )
    assert response.status_code == 422


def test_date_only_confirmations_keep_existing_semantics(client, account, save_balance):
    saved = save_balance("100")
    assert saved["effective_at"] is None
    assert periods(client, account) == []
    assert project(client, account, "2026-02-01T00:00:00Z")["amount"] is None


def test_pending_rows_and_other_accounts_do_not_change_real_totals(
    client, revolut_account, make_csv
):
    zone(client, revolut_account)
    checkpoint(client, revolut_account, "100", "2025-01-01T00:00:00Z")
    checkpoint(client, revolut_account, "90", "2025-02-01T00:00:00Z")
    other = client.post(
        "/api/v1/accounts",
        json={
            "name": "Separate account",
            "currency_code": "EUR",
            "revolut_product": "Current",
        },
    ).json()
    import_rows(client, other, make_csv({"Amount": "999"}))
    import_rows(client, revolut_account, make_csv({}, {"State": "PENDING", "Amount": "500"}))
    period = periods(client, revolut_account)[0]
    assert period["movement_count"] == 1
    assert Decimal(period["real_total"]) == -10
    assert period["evidence"]["pending_rows"] == 1
    assert Decimal(period["required_adjustment"]) == 0


def test_equal_purchases_kept_separate_both_affect_reconciliation(
    client, revolut_account, make_csv
):
    zone(client, revolut_account)
    checkpoint(client, revolut_account, "100", "2025-01-01T00:00:00Z")
    checkpoint(client, revolut_account, "80", "2025-02-01T00:00:00Z")
    import_rows(client, revolut_account, make_csv({}, {}))
    period = periods(client, revolut_account)[0]
    assert period["movement_count"] == 2
    assert Decimal(period["required_adjustment"]) == 0


def test_timezone_change_invalidates_reviewed_evidence(client, revolut_account, make_csv):
    zone(client, revolut_account)
    checkpoint(client, revolut_account, "100", "2025-01-01T00:00:00Z")
    checkpoint(client, revolut_account, "120", "2025-01-02T00:00:00Z")
    import_rows(client, revolut_account, make_csv({"Completed Date": "2025-01-01 00:30:00"}))
    old = approve(client, periods(client, revolut_account)[0]).json()
    assert Decimal(old["active_adjustment"]) == 30
    zone(client, revolut_account, "Europe/Zurich")
    new = periods(client, revolut_account)[0]
    assert new["state"] == "needs_review"
    assert Decimal(new["required_adjustment"]) == 20
    assert approve(client, old).status_code == 409


def test_matapan_timezone_is_used_and_changes_require_review(client, revolut_account, make_csv):
    account = revolut_account
    assert client.get("/api/v1/settings/timezone").json() == {"default_statement_timezone": None}
    assert (
        client.put(
            "/api/v1/settings/timezone",
            json={"default_statement_timezone": "Not/A_Timezone"},
        ).status_code
        == 422
    )
    assert (
        client.put(
            "/api/v1/settings/timezone",
            json={"default_statement_timezone": "UTC"},
        ).status_code
        == 200
    )
    checkpoint(client, account, "100", "2025-01-01T00:00:00Z")
    checkpoint(client, account, "120", "2025-01-02T00:00:00Z")
    import_rows(
        client, account, make_csv({"Completed Date": "2025-01-01 00:30:00", "Amount": "10"})
    )
    initial = periods(client, account)[0]
    assert initial["evidence"]["statement_timezone"] == "UTC"
    assert initial["evidence"]["statement_timezone_source"] == "matapan"
    accepted = approve(client, initial).json()
    assert accepted["state"] == "reconciled"

    changed = client.put(
        "/api/v1/settings/timezone",
        json={"default_statement_timezone": "Europe/Zurich"},
    )
    assert changed.status_code == 200
    revised = periods(client, account)[0]
    assert revised["state"] == "needs_review"
    assert Decimal(revised["required_adjustment"]) == 20
    assert any(item["code"] == "approval_needs_review" for item in revised["coverage_diagnostics"])
    assert approve(client, accepted).status_code == 409


def test_reconciliation_explains_unverified_and_unobserved_intervals(client, account):
    checkpoint(client, account, "100", "2025-01-01T00:00:00Z")
    checkpoint(client, account, "125", "2025-02-01T00:00:00Z")
    response = client.put(
        "/api/v1/settings/timezone",
        json={"default_statement_timezone": "UTC"},
    )
    assert response.status_code == 200
    period = periods(client, account)[0]
    codes = {item["code"] for item in period["coverage_diagnostics"]}
    assert {"coverage_unverified", "no_import_rows", "unresolved_difference"} <= codes
    assert period["coverage"] == "unverified"


def test_reconciliation_labels_activity_gaps_without_claiming_missing_transactions(
    client, revolut_account, make_csv
):
    account = revolut_account
    zone(client, account)
    checkpoint(client, account, "100", "2025-01-01T00:00:00Z")
    checkpoint(client, account, "125", "2025-02-01T00:00:00Z")
    import_rows(client, account, make_csv({"Completed Date": "2025-01-05 10:00:00"}))
    import_rows(client, account, make_csv({"Completed Date": "2025-01-20 10:00:00"}))
    period = periods(client, account)[0]
    gaps = [
        item for item in period["coverage_diagnostics"] if item["code"] == "no_observed_activity"
    ]
    assert len(gaps) == 1
    assert "2025-01-06 through 2025-01-19" in gaps[0]["message"]
    assert "not proof of missing transactions" in gaps[0]["action"]


def test_account_timezone_override_takes_precedence_over_matapan_default(
    client, revolut_account, make_csv
):
    client.put("/api/v1/settings/timezone", json={"default_statement_timezone": "UTC"})
    zone(client, revolut_account, "Europe/Zurich")
    checkpoint(client, revolut_account, "100", "2025-01-02T23:30:00Z")
    checkpoint(client, revolut_account, "110", "2025-01-03T01:00:00Z")
    import_rows(
        client,
        revolut_account,
        make_csv({"Completed Date": "2025-01-03 01:00:00", "Amount": "10"}),
    )
    period = periods(client, revolut_account)[0]
    assert period["evidence"]["statement_timezone"] == "Europe/Zurich"
    assert period["evidence"]["statement_timezone_source"] == "account"
    assert period["movement_count"] == 1


def test_adjustment_write_failure_rolls_back_previous_approval(client, account, monkeypatch):
    from sqlalchemy import event

    from matapan.models import ReconciliationAdjustment

    zone(client, account)
    checkpoint(client, account, "100", "2025-01-01T00:00:00Z")
    checkpoint(client, account, "120", "2025-01-02T00:00:00Z")
    accepted = approve(client, periods(client, account)[0]).json()

    def interrupt(mapper, connection, target):
        raise RuntimeError("Synthetic interruption")

    event.listen(ReconciliationAdjustment, "before_insert", interrupt)
    try:
        with pytest.raises(RuntimeError, match="Synthetic interruption"):
            approve(client, accepted, reverse=True)
    finally:
        event.remove(ReconciliationAdjustment, "before_insert", interrupt)
    unchanged = periods(client, account)[0]
    assert unchanged["history"] == accepted["history"]
    assert unchanged["active_adjustment"] == accepted["active_adjustment"]


def test_restored_database_reproduces_checkpoint_report_and_audit(
    client, account, database_url, tmp_path
):
    import sqlite3

    from sqlalchemy.engine import make_url

    zone(client, account)
    checkpoint(client, account, "100", "2025-01-01T00:00:00Z")
    checkpoint(client, account, "150", "2025-01-02T00:00:00Z")
    approved = approve(client, periods(client, account)[0]).json()
    before = project(client, account, "2025-01-03T00:00:00Z")
    restored_path = tmp_path / "restored.sqlite3"
    with sqlite3.connect(make_url(database_url).database) as source:
        with sqlite3.connect(restored_path) as destination:
            source.backup(destination)
    with TestClient(
        create_app(f"sqlite:///{restored_path.as_posix()}"), base_url="http://localhost"
    ) as restored:
        assert periods(restored, account)[0] == approved
        assert project(restored, account, "2025-01-03T00:00:00Z") == before
