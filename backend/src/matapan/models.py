"""Accounts, immutable financial records, and reviewed import provenance."""

from datetime import date, datetime
from decimal import Decimal
from uuid import uuid4

from sqlalchemy import (
    JSON,
    CheckConstraint,
    Date,
    DateTime,
    ForeignKey,
    Index,
    Integer,
    String,
    UniqueConstraint,
    text,
)
from sqlalchemy.orm import Mapped, mapped_column

from matapan.db import Base, ExactDecimal, UTCDateTime, utc_now


def new_id() -> str:
    return str(uuid4())


class Account(Base):
    __tablename__ = "accounts"
    __table_args__ = (
        CheckConstraint("length(name) BETWEEN 1 AND 120", name="ck_account_name"),
        CheckConstraint("currency_code GLOB '[A-Z][A-Z][A-Z]'", name="ck_account_currency"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    name: Mapped[str] = mapped_column(String(120))
    institution: Mapped[str | None] = mapped_column(String(120), nullable=True)
    currency_code: Mapped[str] = mapped_column(String(3))
    revolut_product: Mapped[str | None] = mapped_column(String(20), nullable=True)
    statement_timezone: Mapped[str | None] = mapped_column(String(100), nullable=True)
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utc_now)


class AccountBalance(Base):
    __tablename__ = "account_balances"
    __table_args__ = (
        CheckConstraint("currency_code GLOB '[A-Z][A-Z][A-Z]'", name="ck_balance_currency"),
        CheckConstraint("length(amount) BETWEEN 1 AND 64", name="ck_balance_amount"),
        CheckConstraint("source IN ('manual', 'revolut')", name="ck_balance_source"),
        CheckConstraint(
            "status IN ('user_confirmed', 'source_reported')", name="ck_balance_status"
        ),
        UniqueConstraint(
            "account_id",
            "currency_code",
            "source",
            "source_timestamp",
            name="uq_balance_source_checkpoint",
        ),
        Index("ix_balance_account_currency_date", "account_id", "currency_code", "as_of_date"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    account_id: Mapped[str] = mapped_column(ForeignKey("accounts.id", ondelete="RESTRICT"))
    currency_code: Mapped[str] = mapped_column(String(3))
    as_of_date: Mapped[date] = mapped_column(Date())
    amount: Mapped[Decimal] = mapped_column(ExactDecimal())
    source: Mapped[str] = mapped_column(String(20), default="manual")
    status: Mapped[str] = mapped_column(String(20), default="user_confirmed")
    note: Mapped[str | None] = mapped_column(String(500), nullable=True)
    recorded_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utc_now)
    source_timestamp: Mapped[str | None] = mapped_column(String(19), nullable=True)
    effective_at: Mapped[datetime | None] = mapped_column(UTCDateTime(), nullable=True)
    effective_offset: Mapped[str | None] = mapped_column(String(6), nullable=True)


class BalanceResolution(Base):
    """Append-only user selections for otherwise conflicting balance evidence."""

    __tablename__ = "balance_resolutions"
    __table_args__ = (
        Index("ix_balance_resolution_group", "account_id", "group_key", "created_at"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    account_id: Mapped[str] = mapped_column(ForeignKey("accounts.id", ondelete="RESTRICT"))
    group_key: Mapped[str] = mapped_column(String(64))
    selected_balance_id: Mapped[str] = mapped_column(ForeignKey("account_balances.id"))
    reason: Mapped[str] = mapped_column(String(500))
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utc_now)


class ImportDecisionAudit(Base):
    """Append-only history for changes to a committed import row's review decision."""

    __tablename__ = "import_decision_audit"
    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    row_id: Mapped[str] = mapped_column(ForeignKey("import_rows.id", ondelete="RESTRICT"))
    previous_action: Mapped[str | None] = mapped_column(String(24), nullable=True)
    previous_transaction_id: Mapped[str | None] = mapped_column(String(36), nullable=True)
    action: Mapped[str] = mapped_column(String(24))
    transaction_id: Mapped[str | None] = mapped_column(String(36), nullable=True)
    previous_posted_at: Mapped[datetime | None] = mapped_column(DateTime(), nullable=True)
    posted_at: Mapped[datetime | None] = mapped_column(DateTime(), nullable=True)
    previous_posted_offset: Mapped[str | None] = mapped_column(String(6), nullable=True)
    posted_offset: Mapped[str | None] = mapped_column(String(6), nullable=True)
    reason: Mapped[str] = mapped_column(String(500))
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utc_now)


class Reconciliation(Base):
    __tablename__ = "reconciliations"
    __table_args__ = (
        UniqueConstraint("opening_balance_id", "closing_balance_id", name="uq_reconciliation_pair"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    account_id: Mapped[str] = mapped_column(ForeignKey("accounts.id", ondelete="RESTRICT"))
    currency_code: Mapped[str] = mapped_column(String(3))
    opening_balance_id: Mapped[str] = mapped_column(ForeignKey("account_balances.id"))
    closing_balance_id: Mapped[str] = mapped_column(ForeignKey("account_balances.id"))
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utc_now)


class ReconciliationAdjustment(Base):
    __tablename__ = "reconciliation_adjustments"
    __table_args__ = (
        CheckConstraint(
            "state IN ('active', 'superseded', 'reversed')", name="ck_adjustment_state"
        ),
        Index(
            "uq_active_reconciliation_adjustment",
            "reconciliation_id",
            unique=True,
            sqlite_where=text("state = 'active'"),
        ),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    reconciliation_id: Mapped[str] = mapped_column(ForeignKey("reconciliations.id"))
    amount: Mapped[Decimal] = mapped_column(ExactDecimal())
    state: Mapped[str] = mapped_column(String(20))
    action: Mapped[str] = mapped_column(String(20))
    reason: Mapped[str] = mapped_column(String(500))
    actor: Mapped[str] = mapped_column(String(40), default="local_user")
    approved_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utc_now)
    replaces_id: Mapped[str | None] = mapped_column(
        ForeignKey("reconciliation_adjustments.id"),
        nullable=True,
    )
    evidence_signature: Mapped[str] = mapped_column(String(64))
    evidence: Mapped[dict] = mapped_column(JSON())


class ImportBatch(Base):
    __tablename__ = "import_batches"
    __table_args__ = (
        UniqueConstraint(
            "account_id", "file_hash", "parser_id", "parser_version", name="uq_import_file_context"
        ),
        CheckConstraint("status IN ('preview', 'committed', 'cancelled')", name="ck_import_status"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    account_id: Mapped[str] = mapped_column(ForeignKey("accounts.id", ondelete="RESTRICT"))
    currency_code: Mapped[str] = mapped_column(String(3))
    product: Mapped[str] = mapped_column(String(20))
    file_hash: Mapped[str] = mapped_column(String(64))
    parser_id: Mapped[str] = mapped_column(String(40))
    parser_version: Mapped[str] = mapped_column(String(20))
    status: Mapped[str] = mapped_column(String(20), default="preview")
    owner_hash: Mapped[str] = mapped_column(String(64))
    observed_start: Mapped[datetime | None] = mapped_column(DateTime(), nullable=True)
    observed_end: Mapped[datetime | None] = mapped_column(DateTime(), nullable=True)
    warnings: Mapped[list[str]] = mapped_column(JSON())
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utc_now)
    expires_at: Mapped[datetime] = mapped_column(UTCDateTime())
    committed_at: Mapped[datetime | None] = mapped_column(UTCDateTime(), nullable=True)


class Transaction(Base):
    __tablename__ = "transactions"
    __table_args__ = (
        Index("ix_transaction_account_date", "account_id", "posted_at"),
        Index("ix_transaction_match", "account_id", "currency_code", "fingerprint"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    account_id: Mapped[str] = mapped_column(ForeignKey("accounts.id", ondelete="RESTRICT"))
    currency_code: Mapped[str] = mapped_column(String(3))
    product: Mapped[str] = mapped_column(String(20))
    transaction_type: Mapped[str] = mapped_column(String(64))
    posted_at: Mapped[datetime] = mapped_column(DateTime())
    posted_offset: Mapped[str | None] = mapped_column(String(6), nullable=True)
    started_at: Mapped[datetime] = mapped_column(DateTime())
    amount: Mapped[Decimal] = mapped_column(ExactDecimal())
    source_amount: Mapped[Decimal] = mapped_column(ExactDecimal())
    fee: Mapped[Decimal] = mapped_column(ExactDecimal())
    source_description: Mapped[str] = mapped_column(String(500))
    fingerprint: Mapped[str] = mapped_column(String(64))
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utc_now)


class ImportRow(Base):
    __tablename__ = "import_rows"
    __table_args__ = (UniqueConstraint("batch_id", "row_number", name="uq_import_row_number"),)

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    batch_id: Mapped[str] = mapped_column(ForeignKey("import_batches.id", ondelete="RESTRICT"))
    row_number: Mapped[int] = mapped_column(Integer())
    classification: Mapped[str] = mapped_column(String(24))
    errors: Mapped[list[str]] = mapped_column(JSON())
    product: Mapped[str] = mapped_column(String(20))
    currency_code: Mapped[str] = mapped_column(String(3))
    source_state: Mapped[str] = mapped_column(String(20))
    transaction_type: Mapped[str] = mapped_column(String(64))
    source_description: Mapped[str] = mapped_column(String(500))
    posted_at: Mapped[datetime | None] = mapped_column(DateTime(), nullable=True)
    corrected_posted_at: Mapped[datetime | None] = mapped_column(DateTime(), nullable=True)
    corrected_offset: Mapped[str | None] = mapped_column(String(6), nullable=True)
    started_at: Mapped[datetime | None] = mapped_column(DateTime(), nullable=True)
    amount: Mapped[Decimal | None] = mapped_column(ExactDecimal(), nullable=True)
    source_amount: Mapped[Decimal | None] = mapped_column(ExactDecimal(), nullable=True)
    fee: Mapped[Decimal | None] = mapped_column(ExactDecimal(), nullable=True)
    balance: Mapped[Decimal | None] = mapped_column(ExactDecimal(), nullable=True)
    fingerprint: Mapped[str | None] = mapped_column(String(64), nullable=True)
    decision: Mapped[str | None] = mapped_column(String(24), nullable=True)
    decided_at: Mapped[datetime | None] = mapped_column(UTCDateTime(), nullable=True)
    transaction_id: Mapped[str | None] = mapped_column(ForeignKey("transactions.id"), nullable=True)
    balance_id: Mapped[str | None] = mapped_column(ForeignKey("account_balances.id"), nullable=True)


class ManualTransactionSource(Base):
    __tablename__ = "manual_transaction_sources"
    transaction_id: Mapped[str] = mapped_column(
        ForeignKey("transactions.id", ondelete="RESTRICT"), primary_key=True
    )
    reason: Mapped[str] = mapped_column(String(500))
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utc_now)
