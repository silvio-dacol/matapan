"""Tests use disposable databases upgraded through the real Alembic migrations."""

import csv
import io
from collections.abc import Iterator
from pathlib import Path

import pytest
from alembic import command
from fastapi.testclient import TestClient

from matapan.db import migration_config
from matapan.importers.parsers.revolut import HEADERS
from matapan.main import create_app


@pytest.fixture
def database_url(tmp_path) -> str:
    url = f"sqlite:///{(tmp_path / 'test.sqlite3').as_posix()}"
    command.upgrade(migration_config(url), "head")
    return url


@pytest.fixture
def client(database_url) -> Iterator[TestClient]:
    with TestClient(create_app(database_url), base_url="http://localhost") as test_client:
        yield test_client


@pytest.fixture
def account(client) -> dict:
    response = client.post(
        "/api/v1/accounts", json={"name": "Synthetic bank account", "currency_code": "CHF"}
    )
    assert response.status_code == 201
    return response.json()


@pytest.fixture
def save_balance(client, account):
    def save(amount: str, as_of: str = "2026-01-31") -> dict:
        response = client.post(
            f"/api/v1/accounts/{account['id']}/cash-balances",
            json={"amount": amount, "as_of_date": as_of, "currency_code": "CHF"},
        )
        assert response.status_code == 201, response.text
        return response.json()

    return save


@pytest.fixture
def revolut_account(client) -> dict:
    response = client.post(
        "/api/v1/accounts",
        json={
            "name": "Synthetic Revolut Current EUR",
            "institution": "Revolut",
            "currency_code": "EUR",
            "revolut_product": "Current",
        },
    )
    assert response.status_code == 201
    return response.json()


@pytest.fixture
def revolut_csv() -> bytes:
    return (Path(__file__).parent / "fixtures" / "revolut_mixed_products.csv").read_bytes()


@pytest.fixture
def overlap_csv() -> bytes:
    return (Path(__file__).parent / "fixtures" / "revolut_overlap.csv").read_bytes()


@pytest.fixture
def make_csv():
    def build(*overrides: dict) -> bytes:
        stream = io.StringIO(newline="")
        writer = csv.DictWriter(stream, fieldnames=HEADERS)
        writer.writeheader()
        for override in overrides:
            row = {
                "Type": "Card Payment",
                "Product": "Current",
                "Started Date": "2025-01-03 10:00:00",
                "Completed Date": "2025-01-03 10:01:00",
                "Description": "Synthetic merchant",
                "Amount": "-10.00",
                "Fee": "0.00",
                "Currency": "EUR",
                "State": "COMPLETED",
                "Balance": "90.00",
            }
            row.update(override)
            writer.writerow(row)
        return stream.getvalue().encode("utf-8")

    return build
