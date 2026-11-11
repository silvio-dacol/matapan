"""As-of evidence selection, missing balances, conflicts, and account isolation."""

from decimal import Decimal

import pytest


def report(client, account, as_of="2026-01-31"):
    response = client.get(f"/api/v1/accounts/{account['id']}/report", params={"as_of": as_of})
    assert response.status_code == 200
    return response.json()


def test_missing_balance_is_unknown_not_zero(client, account):
    result = report(client, account)
    assert result["status"] == "unavailable"
    assert result["amount"] is None
    assert result["balance_status"] == "missing"
    assert result["balance_as_of_date"] is None
    assert result["observation_ids"] == []


@pytest.mark.parametrize("amount", ["0.00", "-42.15", "1234.50"])
def test_confirmed_balance_retains_exact_amount_and_date(client, account, save_balance, amount):
    saved = save_balance(amount)
    result = report(client, account)
    assert result["amount"] == amount
    assert result["balance_as_of_date"] == "2026-01-31"
    assert result["balance_status"] == "user_confirmed"
    assert result["source"] == "manual"
    assert result["observation_ids"] == [saved["id"]]
    assert result["carried_forward"] is False
    assert result["scope"] == "account_cash"
    assert result["status"] == "partial"


def test_future_observations_do_not_change_past_reports(client, account, save_balance):
    earlier = save_balance("100.00", "2026-01-01")
    save_balance("200.00", "2026-01-31")
    result = report(client, account, "2026-01-15")
    assert result["amount"] == "100.00"
    assert result["balance_as_of_date"] == "2026-01-01"
    assert result["observation_ids"] == [earlier["id"]]
    assert result["carried_forward"] is True
    assert any("carried forward" in issue for issue in result["issues"])
    assert report(client, account, "2025-12-31")["amount"] is None


def test_conflicting_observations_are_preserved_and_not_selected(client, account, save_balance):
    first = save_balance("100.00")
    second = save_balance("200.00")
    result = report(client, account)
    assert result["amount"] is None
    assert result["status"] == "unavailable"
    assert result["balance_status"] == "conflicted"
    assert set(result["observation_ids"]) == {first["id"], second["id"]}
    balances = client.get(f"/api/v1/accounts/{account['id']}/cash-balances").json()
    assert {balance["amount"] for balance in balances} == {"100.00", "200.00"}


def test_conflict_resolution_selects_one_observation_and_keeps_both(client, account, save_balance):
    first = save_balance("100.00")
    second = save_balance("200.00")
    response = client.post(
        f"/api/v1/accounts/{account['id']}/balance-resolution",
        json={"selected_balance_id": second["id"], "reason": "Checked the bank app"},
    )
    assert response.status_code == 200, response.text
    assert response.json()["amount"] == "200.00"
    assert response.json()["observation_ids"] == [second["id"]]
    assert {
        item["id"] for item in client.get(f"/api/v1/accounts/{account['id']}/cash-balances").json()
    } == {first["id"], second["id"]}


def test_equal_observations_retain_all_provenance_without_a_conflict(client, account, save_balance):
    first = save_balance("100")
    second = save_balance("100.00")
    result = report(client, account)
    assert Decimal(result["amount"]) == Decimal("100.00")
    assert result["balance_status"] == "user_confirmed"
    assert set(result["observation_ids"]) == {first["id"], second["id"]}


def test_other_accounts_do_not_supply_missing_balances(client, account, save_balance):
    save_balance("100.00")
    other = client.post(
        "/api/v1/accounts", json={"name": "Other synthetic account", "currency_code": "CHF"}
    ).json()
    assert report(client, other)["amount"] is None


def test_report_requires_an_explicit_calendar_date(client, account):
    url = f"/api/v1/accounts/{account['id']}/report"
    assert client.get(url).status_code == 422
    assert client.get(url, params={"as_of": "20260131"}).status_code == 422
