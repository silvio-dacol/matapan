"""Import safety tests use invented statements and real migrated SQLite databases."""

from concurrent.futures import ThreadPoolExecutor
from datetime import timedelta

import pytest
from fastapi.testclient import TestClient
from sqlalchemy import event, select
from sqlalchemy.orm import Session

from matapan.db import make_engine, utc_now
from matapan.main import create_app
from matapan.models import AccountBalance, ImportBatch, ImportRow, Transaction


def preview(client, account, source, filename="synthetic.csv"):
    response = client.post(
        "/api/v1/imports/preview",
        data={"account_id": account["id"]},
        files={"file": (filename, source, "text/csv")},
    )
    assert response.status_code == 200, response.text
    return response.json()


def commit(client, plan, decisions=None, include_balance=True):
    return client.post(
        f"/api/v1/imports/{plan['id']}/commit",
        json={
            "decisions": decisions or [],
            "include_balance": include_balance,
        },
    )


def transactions(client, account):
    return client.get(f"/api/v1/accounts/{account['id']}/transactions").json()


def link_candidates(plan):
    return [
        {
            "row_id": row["id"],
            "action": "link_duplicate",
            "transaction_id": row["candidate_transactions"][0]["id"],
        }
        for row in plan["rows"]
        if row["classification"] == "possible_duplicate"
    ]


@pytest.mark.parametrize("import_status", ["preview", "cancelled", "expired", "committed"])
def test_delete_account_preserves_import_history(
    client, revolut_account, revolut_csv, import_status
):
    plan = preview(client, revolut_account, revolut_csv)
    plan_url = f"/api/v1/imports/{plan['id']}"
    account_url = f"/api/v1/accounts/{revolut_account['id']}"
    if import_status == "cancelled":
        assert client.post(f"{plan_url}/cancel").status_code == 200
    elif import_status == "committed":
        assert commit(client, plan).status_code == 200
    elif import_status == "expired":
        with client.app.state.sessions() as session:
            batch = session.get(ImportBatch, plan["id"])
            batch.expires_at = utc_now() - timedelta(minutes=1)
            session.commit()

    saved_transactions = client.get(f"{account_url}/transactions").json()
    saved_balances = client.get(f"{account_url}/cash-balances").json()
    with client.app.state.sessions() as session:
        saved_rows = session.scalars(
            select(ImportRow.id).where(ImportRow.batch_id == plan["id"]).order_by(ImportRow.id)
        ).all()

    assert client.delete(account_url).status_code == 409
    assert client.get(account_url).json() == revolut_account
    assert client.get(f"{account_url}/transactions").json() == saved_transactions
    assert client.get(f"{account_url}/cash-balances").json() == saved_balances
    with client.app.state.sessions() as session:
        batch = session.get(ImportBatch, plan["id"])
        assert batch is not None
        assert batch.status == ("preview" if import_status == "expired" else import_status)
        assert (
            session.scalars(
                select(ImportRow.id).where(ImportRow.batch_id == plan["id"]).order_by(ImportRow.id)
            ).all()
            == saved_rows
        )


def test_preview_does_not_accept_financial_records(client, revolut_account, revolut_csv):
    plan = preview(client, revolut_account, revolut_csv)
    assert plan["counts"] == {"new": 3, "excluded": 3}
    assert plan["balance"]["amount"] == "86.75"
    assert plan["balance"]["source_timestamp"] == "2025-01-04 10:01:00"
    assert plan["declared_period"] is None
    assert transactions(client, revolut_account) == []
    assert client.get(f"/api/v1/accounts/{revolut_account['id']}/cash-balances").json() == []


def test_confirm_import_and_display_source_balance(client, revolut_account, revolut_csv):
    plan = preview(client, revolut_account, revolut_csv)
    result = commit(client, plan)
    assert result.status_code == 200
    assert result.json()["new_transactions"] == 3
    rows = transactions(client, revolut_account)
    assert len(rows) == 3
    assert any(row["amount"] == "-11.25" for row in rows)
    assert all(row["import_row_ids"] for row in rows)
    report = client.get(f"/api/v1/accounts/{revolut_account['id']}/report?as_of=2025-01-04").json()
    assert report["amount"] == "86.75"
    assert report["balance_status"] == "source_reported"
    assert report["source_timestamp"] == "2025-01-04 10:01:00"
    assert report["status"] == "partial"


def test_repeat_file_under_new_name_and_repeat_commit_are_idempotent(
    client, revolut_account, revolut_csv
):
    plan = preview(client, revolut_account, revolut_csv)
    assert commit(client, plan).status_code == 200
    repeated = preview(client, revolut_account, revolut_csv, "another-name.csv")
    assert repeated["id"] == plan["id"]
    assert repeated["already_imported"] is True
    result = commit(client, repeated).json()
    assert result["already_imported"] is True
    assert result["new_transactions"] == 0
    assert len(transactions(client, revolut_account)) == 3
    assert len(client.get(f"/api/v1/accounts/{revolut_account['id']}/cash-balances").json()) == 1


def test_overlap_requires_review_and_preserves_many_sources(
    client, revolut_account, revolut_csv, overlap_csv
):
    assert commit(client, preview(client, revolut_account, revolut_csv)).status_code == 200
    plan = preview(client, revolut_account, overlap_csv)
    assert plan["counts"] == {"possible_duplicate": 2, "new": 1}
    assert commit(client, plan).status_code == 409
    assert len(transactions(client, revolut_account)) == 3
    result = commit(client, plan, link_candidates(plan))
    assert result.status_code == 200
    assert result.json()["new_transactions"] == 1
    rows = transactions(client, revolut_account)
    assert len(rows) == 4
    assert sum(len(row["import_row_ids"]) == 2 for row in rows) == 2


def test_two_identical_purchases_remain_possible(client, revolut_account, make_csv):
    source = make_csv(
        {},
        {
            "Started Date": "2025-01-03 11:00:00",
            "Completed Date": "2025-01-03 11:01:00",
            "Balance": "80.00",
        },
    )
    plan = preview(client, revolut_account, source)
    assert plan["counts"] == {"new": 2}
    assert commit(client, plan).status_code == 200
    assert len(transactions(client, revolut_account)) == 2
    other = preview(client, revolut_account, make_csv({"Balance": "70.00"}))
    row = other["rows"][0]
    assert row["classification"] == "possible_duplicate"
    result = commit(client, other, [{"row_id": row["id"], "action": "keep_separate"}], False)
    assert result.status_code == 200
    assert len(transactions(client, revolut_account)) == 3


def test_invalid_rows_require_explicit_skip(client, revolut_account, make_csv):
    plan = preview(client, revolut_account, make_csv({}, {"Completed Date": ""}))
    assert plan["counts"] == {"new": 1, "invalid": 1}
    assert commit(client, plan).status_code == 409
    assert transactions(client, revolut_account) == []
    invalid = next(row for row in plan["rows"] if row["classification"] == "invalid")
    assert commit(client, plan, [{"row_id": invalid["id"], "action": "skip"}]).status_code == 200
    assert len(transactions(client, revolut_account)) == 1


def test_absent_balance_stays_unknown_after_commit(client, revolut_account, make_csv):
    plan = preview(client, revolut_account, make_csv({"Balance": ""}))
    assert plan["balance"] is None
    assert commit(client, plan).status_code == 200
    report = client.get(f"/api/v1/accounts/{revolut_account['id']}/report?as_of=2025-01-31").json()
    assert report["amount"] is None
    assert report["balance_status"] == "missing"


def test_products_are_separate_account_contexts(client, revolut_account, revolut_csv):
    assert commit(client, preview(client, revolut_account, revolut_csv)).status_code == 200
    savings = client.post(
        "/api/v1/accounts",
        json={
            "name": "Synthetic Revolut Savings EUR",
            "currency_code": "EUR",
            "revolut_product": "Savings",
        },
    ).json()
    plan = preview(client, savings, revolut_csv)
    assert plan["counts"] == {"new": 2, "excluded": 4}
    assert plan["balance"]["amount"] == "50.02"
    assert commit(client, plan).status_code == 200
    assert len(transactions(client, savings)) == 2
    assert len(transactions(client, revolut_account)) == 3


def test_missing_context_and_wrong_currency_fail_clearly(client, account, revolut_csv):
    response = client.post(
        "/api/v1/imports/preview",
        data={"account_id": account["id"]},
        files={"file": ("synthetic.csv", revolut_csv, "text/csv")},
    )
    assert response.status_code == 422
    assert "revolut_product" in response.json()["detail"]
    configured = client.post(
        "/api/v1/accounts",
        json={
            "name": "Synthetic wrong currency",
            "currency_code": "CHF",
            "revolut_product": "Current",
        },
    ).json()
    assert (
        client.post(
            "/api/v1/imports/preview",
            data={"account_id": configured["id"]},
            files={"file": ("synthetic.csv", revolut_csv, "text/csv")},
        ).status_code
        == 422
    )


def test_conflicting_checkpoint_never_overwrites_existing_balance(
    client, revolut_account, make_csv
):
    original = preview(client, revolut_account, make_csv({}))
    assert commit(client, original).status_code == 200
    conflicting = preview(client, revolut_account, make_csv({"Balance": "89.00"}))
    assert conflicting["balance"]["classification"] == "conflict"
    decisions = link_candidates(conflicting)
    assert commit(client, conflicting, decisions).status_code == 409
    assert len(transactions(client, revolut_account)) == 1
    assert commit(client, conflicting, decisions, False).status_code == 200
    balances = client.get(f"/api/v1/accounts/{revolut_account['id']}/cash-balances").json()
    assert [balance["amount"] for balance in balances] == ["90.00"]


def test_equivalent_checkpoint_is_linked_with_provenance(
    client, revolut_account, make_csv, database_url
):
    source = make_csv({})
    assert commit(client, preview(client, revolut_account, source)).status_code == 200
    same = preview(client, revolut_account, source + b"\r\n")
    assert same["balance"]["classification"] == "duplicate"
    assert commit(client, same, link_candidates(same)).status_code == 200
    engine = make_engine(database_url)
    try:
        with Session(engine) as session:
            assert len(session.scalars(select(AccountBalance)).all()) == 1
            assert (
                len(
                    session.scalars(
                        select(ImportRow).where(ImportRow.balance_id.is_not(None))
                    ).all()
                )
                == 2
            )
    finally:
        engine.dispose()


def test_preview_changes_are_revalidated_before_commit(
    client, revolut_account, revolut_csv, overlap_csv
):
    first = preview(client, revolut_account, revolut_csv)
    stale = preview(client, revolut_account, overlap_csv)
    assert stale["counts"] == {"new": 3}
    assert commit(client, first).status_code == 200
    assert commit(client, stale).status_code == 409
    refreshed = client.get(f"/api/v1/imports/{stale['id']}").json()
    assert refreshed["counts"] == {"possible_duplicate": 2, "new": 1}
    assert commit(client, refreshed, link_candidates(refreshed)).status_code == 200


def test_interrupted_commit_rolls_back_all_financial_writes(client, revolut_account, revolut_csv):
    plan = preview(client, revolut_account, revolut_csv)
    writes = 0

    def interrupt(mapper, connection, target):
        nonlocal writes
        writes += 1
        if writes == 2:
            raise RuntimeError("Simulated interrupted commit")

    event.listen(Transaction, "before_insert", interrupt)
    try:
        with pytest.raises(RuntimeError, match="Simulated interrupted"):
            commit(client, plan)
    finally:
        event.remove(Transaction, "before_insert", interrupt)
    assert transactions(client, revolut_account) == []
    assert client.get(f"/api/v1/accounts/{revolut_account['id']}/cash-balances").json() == []
    refreshed = client.get(f"/api/v1/imports/{plan['id']}").json()
    assert refreshed["status"] == "preview"
    assert all(row["transaction_id"] is None for row in refreshed["rows"])
    assert commit(client, plan).status_code == 200


def test_simultaneous_commits_cannot_duplicate_records(client, revolut_account, revolut_csv):
    plan = preview(client, revolut_account, revolut_csv)
    with ThreadPoolExecutor(max_workers=2) as pool:
        results = list(pool.map(lambda _: commit(client, plan), range(2)))
    assert all(result.status_code == 200 for result in results)
    assert sorted(result.json()["new_transactions"] for result in results) == [0, 3]
    assert len(transactions(client, revolut_account)) == 3


def test_expiry_and_cancel_prevent_commit(client, revolut_account, revolut_csv, database_url):
    plan = preview(client, revolut_account, revolut_csv)
    engine = make_engine(database_url)
    try:
        with Session(engine) as session:
            batch = session.get(ImportBatch, plan["id"])
            batch.expires_at = utc_now() - timedelta(seconds=1)
            session.commit()
    finally:
        engine.dispose()
    assert client.get(f"/api/v1/imports/{plan['id']}").status_code == 410
    assert commit(client, plan).status_code == 410
    renewed = preview(client, revolut_account, revolut_csv)
    assert client.post(f"/api/v1/imports/{renewed['id']}/cancel").status_code == 200
    assert commit(client, renewed).status_code == 409
    assert transactions(client, revolut_account) == []


def test_preview_is_bound_to_its_browser_session(
    client, revolut_account, revolut_csv, database_url
):
    plan = preview(client, revolut_account, revolut_csv)
    with TestClient(create_app(database_url), base_url="http://localhost") as other:
        assert other.get(f"/api/v1/imports/{plan['id']}").status_code == 404
        assert commit(other, plan).status_code == 404
    assert commit(client, plan).status_code == 200


def test_account_scope_prevents_cross_account_duplicate_links(client, revolut_account, revolut_csv):
    original = preview(client, revolut_account, revolut_csv)
    assert commit(client, original).status_code == 200
    other = client.post(
        "/api/v1/accounts",
        json={
            "name": "Another synthetic EUR account",
            "currency_code": "EUR",
            "revolut_product": "Current",
        },
    ).json()
    other_plan = preview(client, other, revolut_csv)
    assert other_plan["counts"] == {"new": 3, "excluded": 3}
    assert commit(client, other_plan).status_code == 200
    different_file = preview(client, revolut_account, revolut_csv + b"\r\n")
    decisions = link_candidates(different_file)
    decisions[0]["transaction_id"] = transactions(client, other)[0]["id"]
    assert commit(client, different_file, decisions).status_code == 422
    assert len(transactions(client, revolut_account)) == 3
    assert len(transactions(client, other)) == 3


def test_later_intraday_snapshot_is_selected_without_overwriting_the_earlier_one(
    client, revolut_account, make_csv
):
    first = preview(client, revolut_account, make_csv({}))
    assert commit(client, first).status_code == 200
    later = preview(
        client,
        revolut_account,
        make_csv(
            {
                "Started Date": "2025-01-03 12:00:00",
                "Completed Date": "2025-01-03 12:01:00",
                "Description": "Another synthetic merchant",
                "Amount": "-5.00",
                "Balance": "85.00",
            }
        ),
    )
    assert commit(client, later).status_code == 200
    report = client.get(f"/api/v1/accounts/{revolut_account['id']}/report?as_of=2025-01-03").json()
    assert report["amount"] == "85.00"
    assert report["source_timestamp"] == "2025-01-03 12:01:00"
    assert len(client.get(f"/api/v1/accounts/{revolut_account['id']}/cash-balances").json()) == 2


def test_pending_rows_never_create_booked_movements(client, revolut_account, make_csv):
    plan = preview(client, revolut_account, make_csv({"State": "PENDING", "Completed Date": ""}))
    assert plan["counts"] == {"excluded": 1}
    assert plan["balance"] is None
    assert commit(client, plan).status_code == 200
    assert transactions(client, revolut_account) == []


def test_committed_duplicate_decision_can_be_revised_and_restored(
    client, revolut_account, make_csv
):
    plan = preview(client, revolut_account, make_csv({"Balance": ""}))
    assert commit(client, plan, include_balance=False).status_code == 200
    row = next(item for item in plan["rows"] if item["classification"] == "new")
    assert len(transactions(client, revolut_account)) == 1
    skipped = client.post(
        f"/api/v1/import-rows/{row['id']}/review",
        json={
            "action": "skip",
            "reason": "This source row was reviewed as an accidental duplicate",
        },
    )
    assert skipped.status_code == 200, skipped.text
    assert transactions(client, revolut_account) == []
    restored = client.post(
        f"/api/v1/import-rows/{row['id']}/review",
        json={"action": "keep_separate", "reason": "Restore the reviewed movement"},
    )
    assert restored.status_code == 200, restored.text
    assert len(transactions(client, revolut_account)) == 1
    history = client.get(f"/api/v1/import-rows/{row['id']}/decision-history").json()
    assert [item["action"] for item in history] == ["skip", "keep_separate"]


def test_manual_movement_is_reviewable_and_linked_import_keeps_provenance(
    client, revolut_account, make_csv
):
    added = client.post(
        f"/api/v1/accounts/{revolut_account['id']}/transactions/manual",
        json={
            "posted_at": "2025-01-03T10:01:00+01:00",
            "amount": "-10.00",
            "description": "Synthetic merchant",
            "reason": "Cash receipt entered manually",
        },
    )
    assert added.status_code == 200, added.text
    assert added.json()["source"] == "manual"
    plan = preview(client, revolut_account, make_csv({"Balance": ""}))
    row = next(item for item in plan["rows"] if item["classification"] == "possible_duplicate")
    assert row["candidate_transactions"][0]["id"] == added.json()["id"]
    result = commit(
        client,
        plan,
        [
            {
                "row_id": row["id"],
                "action": "link_duplicate",
                "transaction_id": added.json()["id"],
            }
        ],
        include_balance=False,
    )
    assert result.status_code == 200, result.text
    activity = transactions(client, revolut_account)
    assert len(activity) == 1
    assert activity[0]["source"] == "manual"
    assert activity[0]["manual_reason"] == "Cash receipt entered manually"
    assert len(activity[0]["import_row_ids"]) == 1


def test_import_row_time_correction_keeps_original_source_timestamp(
    client, revolut_account, make_csv
):
    plan = preview(client, revolut_account, make_csv({"Balance": ""}))
    assert commit(client, plan, include_balance=False).status_code == 200
    row = next(item for item in plan["rows"] if item["classification"] == "new")
    corrected = client.post(
        f"/api/v1/import-rows/{row['id']}/correct-time",
        json={"posted_at": "2025-01-03T11:01:00+02:00", "reason": "Verified against bank receipt"},
    )
    assert corrected.status_code == 200, corrected.text
    with client.app.state.sessions() as session:
        source = session.get(ImportRow, row["id"])
        assert source.posted_at.isoformat(sep=" ") == "2025-01-03 10:01:00"
        assert source.corrected_posted_at.isoformat(sep=" ") == "2025-01-03 11:01:00"
        assert source.corrected_offset == "+02:00"
