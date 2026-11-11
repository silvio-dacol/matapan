"""Migration safety, SQLite referential integrity, and exact money storage."""

from datetime import date
from decimal import Decimal
from uuid import uuid4

import pytest
from alembic import command
from fastapi.testclient import TestClient
from sqlalchemy import text
from sqlalchemy.exc import IntegrityError, StatementError
from sqlalchemy.orm import Session

from matapan.db import make_engine, migration_config, require_current_schema
from matapan.main import create_app
from matapan.models import Account, AccountBalance


def test_unmigrated_database_has_actionable_startup_error(tmp_path):
    url = f"sqlite:///{(tmp_path / 'empty.sqlite3').as_posix()}"
    with pytest.raises(RuntimeError, match="alembic upgrade head"):
        with TestClient(create_app(url), base_url="http://localhost"):
            pass


def test_migration_can_be_reapplied_without_deleting_data(database_url):
    engine = make_engine(database_url)
    try:
        with Session(engine) as session:
            account = Account(name="Retained synthetic account", currency_code="CHF")
            session.add(account)
            session.commit()
            account_id = account.id
        command.upgrade(migration_config(database_url), "head")
        require_current_schema(engine)
        with Session(engine) as session:
            assert session.get(Account, account_id).name == "Retained synthetic account"
        command.check(migration_config(database_url))
    finally:
        engine.dispose()


def test_sqlite_enforces_foreign_keys(database_url):
    engine = make_engine(database_url)
    try:
        with Session(engine) as session:
            session.add(
                AccountBalance(
                    account_id=str(uuid4()),
                    currency_code="CHF",
                    as_of_date=date(2026, 1, 31),
                    amount=Decimal("10.00"),
                )
            )
            with pytest.raises(IntegrityError):
                session.commit()
            session.rollback()
    finally:
        engine.dispose()


def test_exact_decimal_is_text_in_sqlite(database_url):
    engine = make_engine(database_url)
    try:
        with Session(engine) as session:
            account = Account(name="Precision test", currency_code="CHF")
            session.add(account)
            session.flush()
            observation = AccountBalance(
                account_id=account.id,
                currency_code="CHF",
                as_of_date=date(2026, 1, 31),
                amount=Decimal("9007199254740993.12345678"),
            )
            session.add(observation)
            session.commit()
            amount, storage_type = session.execute(
                text("SELECT amount, typeof(amount) FROM account_balances")
            ).one()
            assert (amount, storage_type) == ("9007199254740993.12345678", "text")
            session.expire_all()
            assert session.get(AccountBalance, observation.id).amount == Decimal(amount)
    finally:
        engine.dispose()


def test_persistence_layer_rejects_float_money(database_url):
    engine = make_engine(database_url)
    try:
        with Session(engine) as session:
            account = Account(name="Float rejection test", currency_code="CHF")
            session.add(account)
            session.flush()
            session.add(
                AccountBalance(
                    account_id=account.id,
                    currency_code="CHF",
                    as_of_date=date(2026, 1, 31),
                    amount=0.1,
                )
            )
            with pytest.raises(StatementError, match="finite Decimal"):
                session.commit()
            session.rollback()
    finally:
        engine.dispose()


def test_import_migration_preserves_existing_manual_balances(tmp_path):
    url = f"sqlite:///{(tmp_path / 'existing.sqlite3').as_posix()}"
    config = migration_config(url)
    command.upgrade(config, "0001_account_balances")
    account_id, balance_id = str(uuid4()), str(uuid4())
    engine = make_engine(url)
    try:
        with engine.begin() as connection:
            connection.execute(
                text(
                    "INSERT INTO accounts (id, name, currency_code, created_at) "
                    "VALUES (:id, 'Existing synthetic account', 'CHF', '2026-01-31 12:00:00')"
                ),
                {"id": account_id},
            )
            connection.execute(
                text(
                    "INSERT INTO account_balances "
                    "(id, account_id, currency_code, as_of_date, amount, source, status, "
                    "note, recorded_at) "
                    "VALUES (:id, :account, 'CHF', '2026-01-31', :amount, 'manual', "
                    "'user_confirmed', 'Preserve this observation', '2026-01-31 12:00:00')"
                ),
                {"id": balance_id, "account": account_id, "amount": "9007199254740993.12345678"},
            )
        command.upgrade(config, "head")
        with Session(engine) as session:
            observation = session.get(AccountBalance, balance_id)
            assert observation.amount == Decimal("9007199254740993.12345678")
            assert observation.source == "manual"
            assert observation.status == "user_confirmed"
            assert observation.note == "Preserve this observation"
            assert observation.source_timestamp is None
            assert observation.effective_at is None
            assert observation.effective_offset is None
            assert session.get(Account, account_id).revolut_product is None
            assert session.get(Account, account_id).statement_timezone is None
        command.check(config)
    finally:
        engine.dispose()


def test_checkpoint_migration_preserves_imported_evidence(tmp_path):
    url = f"sqlite:///{(tmp_path / 'imported.sqlite3').as_posix()}"
    config = migration_config(url)
    command.upgrade(config, "0002_revolut_imports")
    engine = make_engine(url)
    try:
        with engine.begin() as connection:
            connection.execute(
                text(
                    "INSERT INTO accounts (id, name, currency_code, revolut_product, created_at) "
                    "VALUES ('account', 'Synthetic imported account', 'EUR', 'Current', "
                    "'2025-01-03 12:00:00')"
                )
            )
            connection.execute(
                text(
                    "INSERT INTO account_balances "
                    "(id, account_id, currency_code, as_of_date, amount, source, status, "
                    "recorded_at, source_timestamp) VALUES "
                    "('balance', 'account', 'EUR', '2025-01-03', '90.12345678', 'revolut', "
                    "'source_reported', '2025-01-03 12:00:00', '2025-01-03 10:01:00')"
                )
            )
            connection.execute(
                text(
                    "INSERT INTO import_batches "
                    "(id, account_id, currency_code, product, file_hash, parser_id, "
                    "parser_version, "
                    "status, owner_hash, warnings, created_at, expires_at) VALUES "
                    "('batch', 'account', 'EUR', 'Current', 'hash', 'revolut_bank_csv_en', '1', "
                    "'committed', 'owner', '[]', '2025-01-03 12:00:00', '2025-01-03 12:30:00')"
                )
            )
            connection.execute(
                text(
                    "INSERT INTO transactions "
                    "(id, account_id, currency_code, product, transaction_type, posted_at, "
                    "started_at, amount, source_amount, fee, source_description, "
                    "fingerprint, created_at) VALUES "
                    "('transaction', 'account', 'EUR', 'Current', 'Card Payment', "
                    "'2025-01-03 10:01:00', '2025-01-03 10:00:00', '-10', '-10', '0', "
                    "'Synthetic merchant', 'fingerprint', '2025-01-03 12:00:00')"
                )
            )
            connection.execute(
                text(
                    "INSERT INTO import_rows "
                    "(id, batch_id, row_number, classification, errors, product, currency_code, "
                    "source_state, transaction_type, source_description, decision, transaction_id, "
                    "balance_id) VALUES ('row', 'batch', 2, 'new', '[]', 'Current', 'EUR', "
                    "'COMPLETED', 'Card Payment', 'Synthetic merchant', 'import_new', "
                    "'transaction', 'balance')"
                )
            )
            before = {
                "import_batches": connection.execute(
                    text("SELECT id, account_id, file_hash, status FROM import_batches")
                ).all(),
                "transactions": connection.execute(
                    text("SELECT id, account_id, amount, fingerprint FROM transactions")
                ).all(),
                "import_rows": connection.execute(
                    text(
                        "SELECT id, batch_id, decision, transaction_id, balance_id FROM import_rows"
                    )
                ).all(),
            }
        command.upgrade(config, "head")
        with engine.connect() as connection:
            after = {
                "import_batches": connection.execute(
                    text("SELECT id, account_id, file_hash, status FROM import_batches")
                ).all(),
                "transactions": connection.execute(
                    text("SELECT id, account_id, amount, fingerprint FROM transactions")
                ).all(),
                "import_rows": connection.execute(
                    text(
                        "SELECT id, batch_id, decision, transaction_id, balance_id FROM import_rows"
                    )
                ).all(),
            }
            assert after == before
            assert (
                connection.execute(
                    text("SELECT posted_offset FROM transactions WHERE id = 'transaction'")
                ).scalar_one()
                is None
            )
            assert connection.execute(
                text("SELECT amount, source_timestamp, effective_at FROM account_balances")
            ).one() == ("90.12345678", "2025-01-03 10:01:00", None)
        command.check(config)
    finally:
        engine.dispose()
