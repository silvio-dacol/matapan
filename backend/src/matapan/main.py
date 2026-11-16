"""Local HTTP API for reviewed account imports and cash reconciliation."""

import hashlib
import re
import secrets
from collections.abc import Iterator
from contextlib import asynccontextmanager
from datetime import date, datetime
from decimal import Decimal
from typing import Annotated, Literal
from uuid import UUID

from fastapi import (
    Depends,
    FastAPI,
    Form,
    HTTPException,
    Query,
    Request,
    Response,
    UploadFile,
    status,
)
from fastapi.responses import JSONResponse
from pydantic import (
    AwareDatetime,
    BaseModel,
    BeforeValidator,
    ConfigDict,
    Field,
    field_validator,
    model_validator,
)
from sqlalchemy import or_, select, text
from sqlalchemy.exc import IntegrityError, SQLAlchemyError
from sqlalchemy.orm import Session, sessionmaker
from starlette.middleware.trustedhost import TrustedHostMiddleware

from matapan import __version__, imports
from matapan.db import database_url, make_engine, require_current_schema, utc_now
from matapan.importers.parsers.revolut import MAX_FILE_BYTES, StatementError
from matapan.models import (
    Account,
    AccountBalance,
    AppSetting,
    BalanceResolution,
    ImportBatch,
    ImportDecisionAudit,
    ImportRow,
    ManualTransactionSource,
    Transaction,
)
from matapan.reports import account_report, balance_group_key
from matapan.services import reconciliation_service as reconciliation


def calendar_date(value):
    if not isinstance(value, str) or not re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}", value):
        raise ValueError("Send a calendar date in YYYY-MM-DD format")
    return date.fromisoformat(value)


CalendarDate = Annotated[date, BeforeValidator(calendar_date)]


def instant(value):
    if not isinstance(value, str) or not re.fullmatch(
        r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(Z|[+-][0-9]{2}:[0-9]{2})", value
    ):
        raise ValueError("Send an ISO timestamp with seconds and an explicit UTC offset")
    parsed = datetime.fromisoformat(value)
    if value[-1] != "Z" and int(value[-2:]) > 59:
        raise ValueError("UTC offset minutes must be between 00 and 59")
    if abs(parsed.utcoffset().total_seconds()) > 14 * 3600:
        raise ValueError("UTC offset must be between -14:00 and +14:00")
    return parsed


Instant = Annotated[AwareDatetime, BeforeValidator(instant)]


class RequestModel(BaseModel):
    model_config = ConfigDict(extra="forbid", str_strip_whitespace=True)


class AccountCreate(RequestModel):
    name: str = Field(min_length=1, max_length=120)
    institution: str | None = Field(default=None, min_length=1, max_length=120)
    currency_code: str = Field(pattern=r"^[A-Z]{3}$")
    revolut_product: Literal["Current", "Savings"] | None = None

    @field_validator("currency_code", mode="before")
    @classmethod
    def normalize_currency(cls, value):
        return value.strip().upper() if isinstance(value, str) else value


class AccountRead(BaseModel):
    model_config = ConfigDict(from_attributes=True)

    id: UUID
    name: str
    institution: str | None
    currency_code: str
    revolut_product: Literal["Current", "Savings"] | None
    created_at: datetime
    statement_timezone: str | None


class BalanceCreate(RequestModel):
    currency_code: str = Field(pattern=r"^[A-Z]{3}$")
    as_of_date: CalendarDate
    amount: Decimal = Field(max_digits=28, decimal_places=8, allow_inf_nan=False)
    note: str | None = Field(default=None, max_length=500)
    effective_at: Instant | None = None

    @model_validator(mode="after")
    def matching_date(self):
        if self.effective_at and self.effective_at.date() != self.as_of_date:
            raise ValueError("as_of_date must match the local date of effective_at")
        return self

    @field_validator("currency_code", mode="before")
    @classmethod
    def normalize_currency(cls, value):
        return value.strip().upper() if isinstance(value, str) else value

    @field_validator("amount", mode="before", json_schema_input_type=str)
    @classmethod
    def exact_amount(cls, value):
        if not isinstance(value, str) or not re.fullmatch(
            r"-?(0|[1-9][0-9]*)(\.[0-9]{1,8})?", value
        ):
            raise ValueError("Send amount as a decimal string, for example '1234.50'")
        return Decimal(value)


class BalanceConflictDecision(RequestModel):
    selected_balance_id: UUID
    reason: str = Field(min_length=1, max_length=500)


class ImportRowReview(RequestModel):
    action: Literal["keep_separate", "link_duplicate", "skip"]
    transaction_id: UUID | None = None
    reason: str = Field(min_length=1, max_length=500)

    @model_validator(mode="after")
    def require_target(self):
        if (self.action == "link_duplicate") != (self.transaction_id is not None):
            raise ValueError("transaction_id is required only for link_duplicate")
        return self


class ManualMovementCreate(RequestModel):
    posted_at: Instant
    amount: Decimal = Field(max_digits=28, decimal_places=8, allow_inf_nan=False)
    description: str = Field(min_length=1, max_length=500)
    reason: str = Field(min_length=1, max_length=500)

    @field_validator("amount", mode="before", json_schema_input_type=str)
    @classmethod
    def exact_amount(cls, value):
        if not isinstance(value, str) or not re.fullmatch(
            r"-?(0|[1-9][0-9]*)(\.[0-9]{1,8})?", value
        ):
            raise ValueError("Send amount as a decimal string, for example '12.50'")
        if Decimal(value) == 0:
            raise ValueError("Manual movement amount must be non-zero")
        return Decimal(value)


class ImportTimeCorrection(RequestModel):
    posted_at: Instant
    reason: str = Field(min_length=1, max_length=500)


class BalanceRead(BaseModel):
    model_config = ConfigDict(from_attributes=True)

    id: UUID
    account_id: UUID
    currency_code: str
    as_of_date: date
    amount: Decimal
    source: Literal["manual", "revolut"]
    status: Literal["user_confirmed", "source_reported"]
    source_timestamp: str | None
    note: str | None
    recorded_at: datetime
    effective_at: datetime | None
    effective_offset: str | None


class StatementTimezone(RequestModel):
    statement_timezone: str | None = Field(default=None, max_length=100)

    @field_validator("statement_timezone", mode="before")
    @classmethod
    def blank_override_is_unset(cls, value):
        return value.strip() or None if isinstance(value, str) else value


class MatapanTimezoneRead(BaseModel):
    default_statement_timezone: str | None


class MatapanTimezoneUpdate(RequestModel):
    default_statement_timezone: str = Field(min_length=1, max_length=100)


class AdjustmentDecision(RequestModel):
    signature: str = Field(pattern=r"^[a-f0-9]{64}$")
    reason: str = Field(min_length=1, max_length=500)


class AccountReport(BaseModel):
    account_id: UUID
    currency_code: str
    as_of_date: date
    scope: Literal["account_cash"]
    status: Literal["partial", "unavailable"]
    amount: Decimal | None
    balance_as_of_date: date | None
    balance_status: Literal["user_confirmed", "source_reported", "missing", "conflicted"]
    source: Literal["manual", "revolut"] | None
    source_timestamp: str | None
    balance_effective_at: datetime | None
    observation_ids: list[UUID]
    carried_forward: bool
    issues: list[str]
    resolution: dict | None = None


class HealthRead(BaseModel):
    status: Literal["ok"]
    database: Literal["ok"]
    version: str


class RowDecision(RequestModel):
    row_id: UUID
    action: Literal["keep_separate", "link_duplicate", "skip"]
    transaction_id: UUID | None = None

    @model_validator(mode="after")
    def require_target(self):
        if (self.action == "link_duplicate") != (self.transaction_id is not None):
            raise ValueError("transaction_id is required only for link_duplicate")
        return self


class ImportCommit(RequestModel):
    decisions: list[RowDecision] = Field(default_factory=list, max_length=10_000)
    include_balance: bool = True


class TransactionRead(BaseModel):
    model_config = ConfigDict(from_attributes=True)

    id: UUID
    account_id: UUID
    currency_code: str
    product: str
    transaction_type: str
    posted_at: datetime
    posted_offset: str | None = None
    started_at: datetime
    amount: Decimal
    source_amount: Decimal
    fee: Decimal
    source_description: str
    import_row_ids: list[UUID] = Field(default_factory=list)
    source: Literal["import", "manual"] = "import"
    manual_reason: str | None = None


SESSION_COOKIE = "matapan_import_session"


def import_owner(request: Request, response: Response | None = None) -> str:
    token = request.cookies.get(SESSION_COOKIE, "")
    if not re.fullmatch(r"[A-Za-z0-9_-]{43}", token):
        if response is None:
            raise imports.ImportProblem(404, "Import preview not found in this browser session")
        token = secrets.token_urlsafe(32)
        response.set_cookie(SESSION_COOKIE, token, httponly=True, samesite="strict", max_age=86_400)
    return hashlib.sha256(token.encode()).hexdigest()


def get_session(request: Request) -> Iterator[Session]:
    with request.app.state.sessions() as session:
        yield session


DatabaseSession = Annotated[Session, Depends(get_session)]
PageLimit = Annotated[int, Query(ge=1, le=100)]
PageOffset = Annotated[int, Query(ge=0)]


def get_account(session: Session, account_id: UUID) -> Account:
    account = session.get(Account, str(account_id))
    if account is None:
        raise HTTPException(status_code=404, detail="Account not found")
    return account


def create_app(url: str | None = None) -> FastAPI:
    @asynccontextmanager
    async def lifespan(application: FastAPI):
        engine = make_engine(url or database_url())
        try:
            require_current_schema(engine)
            application.state.sessions = sessionmaker(engine, expire_on_commit=False)
            yield
        finally:
            engine.dispose()

    application = FastAPI(
        title="Matapan API",
        version=__version__,
        description=(
            "Local accounts, dated balance evidence, and timed cash reconciliation. "
            "Revolut CSV imports require a preview and explicit confirmation. "
            "Send monetary amounts as decimal strings."
        ),
        lifespan=lifespan,
    )
    application.add_middleware(
        TrustedHostMiddleware, allowed_hosts=["localhost", "127.0.0.1", "[::1]"]
    )

    @application.exception_handler(imports.ImportProblem)
    async def handle_import_problem(request: Request, error: imports.ImportProblem):
        return JSONResponse(status_code=error.status_code, content={"detail": error.detail})

    @application.exception_handler(StatementError)
    async def handle_statement_error(request: Request, error: StatementError):
        return JSONResponse(status_code=422, content={"detail": str(error)})

    @application.get("/health", response_model=HealthRead, tags=["Health"])
    def health(session: DatabaseSession):
        try:
            session.execute(text("SELECT 1"))
        except SQLAlchemyError as error:
            raise HTTPException(status_code=503, detail="Database unavailable") from error
        return {"status": "ok", "database": "ok", "version": __version__}

    @application.post(
        "/api/v1/accounts",
        response_model=AccountRead,
        status_code=status.HTTP_201_CREATED,
        tags=["Accounts"],
    )
    def create_account(payload: AccountCreate, session: DatabaseSession):
        account = Account(**payload.model_dump())
        session.add(account)
        session.commit()
        return account

    @application.get("/api/v1/accounts", response_model=list[AccountRead], tags=["Accounts"])
    def list_accounts(session: DatabaseSession, limit: PageLimit = 100, offset: PageOffset = 0):
        return session.scalars(
            select(Account).order_by(Account.created_at, Account.id).limit(limit).offset(offset)
        ).all()

    @application.get("/api/v1/accounts/{account_id}", response_model=AccountRead, tags=["Accounts"])
    def read_account(account_id: UUID, session: DatabaseSession):
        return get_account(session, account_id)

    @application.delete(
        "/api/v1/accounts/{account_id}",
        status_code=status.HTTP_204_NO_CONTENT,
        tags=["Accounts"],
        summary="Delete an empty account",
        description=(
            "Permanently delete an account only when it has no balances, transactions, "
            "or import history (including cancelled or expired previews). "
            "Linked records are preserved and block deletion."
        ),
        responses={
            404: {"description": "Account not found"},
            409: {"description": "Account has linked records and cannot be deleted"},
        },
    )
    def delete_account(account_id: UUID, session: DatabaseSession):
        account = get_account(session, account_id)
        session.delete(account)
        try:
            # Database RESTRICT constraints protect linked data, including concurrent writes.
            session.commit()
        except IntegrityError as error:
            session.rollback()
            raise HTTPException(
                status_code=409,
                detail=(
                    "Account has balances, transactions, or import history and cannot be deleted."
                ),
            ) from error
        return Response(status_code=status.HTTP_204_NO_CONTENT)

    @application.post(
        "/api/v1/accounts/{account_id}/cash-balances",
        response_model=BalanceRead,
        status_code=status.HTTP_201_CREATED,
        tags=["Balances"],
        summary="Confirm a cash balance, optionally at a precise instant",
    )
    def create_balance(account_id: UUID, payload: BalanceCreate, session: DatabaseSession):
        account = reconciliation.lock_account(session, str(account_id))
        if payload.currency_code != account.currency_code:
            raise HTTPException(
                status_code=422, detail="Balance currency must match account currency"
            )
        values = payload.model_dump()
        if payload.effective_at:
            offset = payload.effective_at.strftime("%z")
            values["effective_offset"] = f"{offset[:3]}:{offset[3:]}"
        observation = AccountBalance(account_id=account.id, **values)
        session.add(observation)
        session.flush()
        reconciliation.sync_periods(session, account)
        session.commit()
        session.refresh(observation)
        return observation

    @application.post("/api/v1/accounts/{account_id}/transactions/manual", tags=["Transactions"])
    def create_manual_movement(
        account_id: UUID, payload: ManualMovementCreate, session: DatabaseSession
    ):
        account = reconciliation.lock_account(session, str(account_id))
        local_time = payload.posted_at.replace(tzinfo=None)
        offset = payload.posted_at.strftime("%z")
        offset = f"{offset[:3]}:{offset[3:]}"
        transaction = Transaction(
            account_id=account.id,
            currency_code=account.currency_code,
            product=account.revolut_product or "Manual",
            transaction_type="manual_movement",
            posted_at=local_time,
            posted_offset=offset,
            started_at=local_time,
            amount=payload.amount,
            source_amount=payload.amount,
            fee=Decimal(0),
            source_description=payload.description,
            fingerprint=hashlib.sha256(f"manual:{secrets.token_urlsafe(24)}".encode()).hexdigest(),
        )
        session.add(transaction)
        session.flush()
        session.add(ManualTransactionSource(transaction_id=transaction.id, reason=payload.reason))
        reconciliation.sync_periods(session, account)
        session.commit()
        result = TransactionRead.model_validate(transaction)
        result.source = "manual"
        result.manual_reason = payload.reason
        return result

    @application.put(
        "/api/v1/accounts/{account_id}/statement-timezone",
        response_model=AccountRead,
        tags=["Reconciliation"],
    )
    def set_statement_timezone(
        account_id: UUID, payload: StatementTimezone, session: DatabaseSession
    ):
        if payload.statement_timezone:
            reconciliation.timezone(payload.statement_timezone)
        account = reconciliation.lock_account(session, str(account_id))
        account.statement_timezone = payload.statement_timezone
        session.commit()
        return account

    @application.get(
        "/api/v1/settings/timezone", response_model=MatapanTimezoneRead, tags=["Settings"]
    )
    def read_matapan_timezone(session: DatabaseSession):
        setting = session.get(AppSetting, "default_statement_timezone")
        return {"default_statement_timezone": setting.value if setting else None}

    @application.put(
        "/api/v1/settings/timezone", response_model=MatapanTimezoneRead, tags=["Settings"]
    )
    def set_matapan_timezone(payload: MatapanTimezoneUpdate, session: DatabaseSession):
        reconciliation.timezone(payload.default_statement_timezone)
        setting = session.get(AppSetting, "default_statement_timezone")
        if setting is None:
            setting = AppSetting(
                key="default_statement_timezone", value=payload.default_statement_timezone
            )
            session.add(setting)
        else:
            setting.value = payload.default_statement_timezone
        session.commit()
        return {"default_statement_timezone": setting.value}

    @application.get("/api/v1/accounts/{account_id}/reconciliations", tags=["Reconciliation"])
    def list_reconciliations(account_id: UUID, session: DatabaseSession):
        return reconciliation.list_periods(session, get_account(session, account_id))

    @application.post(
        "/api/v1/reconciliations/{reconciliation_id}/accept-adjustment", tags=["Reconciliation"]
    )
    def accept_adjustment(
        reconciliation_id: UUID, payload: AdjustmentDecision, session: DatabaseSession
    ):
        return reconciliation.decide(
            session, str(reconciliation_id), payload.signature, payload.reason
        )

    @application.post(
        "/api/v1/reconciliations/{reconciliation_id}/reverse-adjustment", tags=["Reconciliation"]
    )
    def reverse_adjustment(
        reconciliation_id: UUID, payload: AdjustmentDecision, session: DatabaseSession
    ):
        return reconciliation.decide(
            session, str(reconciliation_id), payload.signature, payload.reason, reverse=True
        )

    @application.get("/api/v1/accounts/{account_id}/projection", tags=["Reports"])
    def read_projection(account_id: UUID, at: Instant, session: DatabaseSession):
        return reconciliation.projection(session, get_account(session, account_id), at)

    @application.get(
        "/api/v1/accounts/{account_id}/cash-balances",
        response_model=list[BalanceRead],
        tags=["Balances"],
    )
    def list_balances(
        account_id: UUID, session: DatabaseSession, limit: PageLimit = 100, offset: PageOffset = 0
    ):
        get_account(session, account_id)
        return session.scalars(
            select(AccountBalance)
            .where(AccountBalance.account_id == str(account_id))
            .order_by(
                AccountBalance.as_of_date.desc(), AccountBalance.recorded_at, AccountBalance.id
            )
            .limit(limit)
            .offset(offset)
        ).all()

    @application.post(
        "/api/v1/accounts/{account_id}/balance-resolution",
        response_model=AccountReport,
        tags=["Balances"],
    )
    def resolve_balance_conflict(
        account_id: UUID, payload: BalanceConflictDecision, session: DatabaseSession
    ):
        account = reconciliation.lock_account(session, str(account_id))
        selected = session.get(AccountBalance, str(payload.selected_balance_id))
        if selected is None or selected.account_id != account.id:
            raise HTTPException(status_code=422, detail="Select evidence for this account.")
        observations = session.scalars(
            select(AccountBalance).where(
                AccountBalance.account_id == account.id,
                AccountBalance.currency_code == account.currency_code,
                AccountBalance.as_of_date == selected.as_of_date,
            )
        ).all()
        manual = [item for item in observations if item.source == "manual"]
        if manual and all(item.effective_at is not None for item in manual):
            latest = max(item.effective_at for item in manual)
            observations = [item for item in manual if item.effective_at == latest]
        elif not manual:
            latest = max(item.source_timestamp or "" for item in observations)
            observations = [
                item for item in observations if (item.source_timestamp or "") == latest
            ]
        if len({item.amount for item in observations}) < 2:
            raise HTTPException(status_code=409, detail="These observations do not conflict.")
        if selected.id not in {item.id for item in observations}:
            raise HTTPException(
                status_code=422, detail="Select one of the conflicting observations."
            )
        session.add(
            BalanceResolution(
                account_id=account.id,
                group_key=balance_group_key(account.id, observations),
                selected_balance_id=selected.id,
                reason=payload.reason,
            )
        )
        session.commit()
        return account_report(session, account, selected.as_of_date)

    @application.get(
        "/api/v1/accounts/{account_id}/report", response_model=AccountReport, tags=["Reports"]
    )
    def read_report(account_id: UUID, as_of: CalendarDate, session: DatabaseSession):
        return account_report(session, get_account(session, account_id), as_of)

    @application.post("/api/v1/imports/preview", tags=["Imports"])
    def preview_import(
        account_id: Annotated[UUID, Form()],
        file: UploadFile,
        request: Request,
        response: Response,
        session: DatabaseSession,
    ):
        content = file.file.read(MAX_FILE_BYTES + 1)
        if len(content) > MAX_FILE_BYTES:
            raise HTTPException(status_code=413, detail="Statement exceeds the 10 MiB file limit")
        return imports.create_preview(
            session, str(account_id), content, import_owner(request, response)
        )

    @application.get("/api/v1/accounts/{account_id}/import-history", tags=["Imports"])
    def import_history(account_id: UUID, session: DatabaseSession):
        get_account(session, account_id)
        batches = session.scalars(
            select(ImportBatch)
            .where(ImportBatch.account_id == str(account_id))
            .order_by(ImportBatch.created_at.desc(), ImportBatch.id.desc())
        ).all()
        return [
            {
                "id": batch.id,
                "status": batch.status,
                "created_at": batch.created_at,
                "committed_at": batch.committed_at,
                "observed_start": batch.observed_start,
                "observed_end": batch.observed_end,
                "parser_id": batch.parser_id,
                "warnings": batch.warnings,
                "rows": [
                    {
                        "id": row.id,
                        "row_number": row.row_number,
                        "classification": row.classification,
                        "decision": row.decision,
                        "transaction_id": row.transaction_id,
                        "description": row.source_description,
                        "posted_at": row.posted_at,
                        "corrected_posted_at": row.corrected_posted_at,
                        "corrected_offset": row.corrected_offset,
                        "amount": format(row.amount, "f") if row.amount is not None else None,
                        "candidates": [
                            {
                                "id": item.id,
                                "description": item.source_description,
                                "amount": format(item.amount, "f"),
                            }
                            for item in imports.match_candidates(session, batch, [row]).get(
                                row.fingerprint, []
                            )
                        ],
                    }
                    for row in session.scalars(
                        select(ImportRow)
                        .where(ImportRow.batch_id == batch.id)
                        .order_by(ImportRow.row_number)
                    )
                ],
            }
            for batch in batches
        ]

    @application.get("/api/v1/import-rows/{row_id}/decision-history", tags=["Imports"])
    def import_decision_history(row_id: UUID, session: DatabaseSession):
        if session.get(ImportRow, str(row_id)) is None:
            raise HTTPException(status_code=404, detail="Import row not found")
        return session.scalars(
            select(ImportDecisionAudit)
            .where(ImportDecisionAudit.row_id == str(row_id))
            .order_by(ImportDecisionAudit.created_at, ImportDecisionAudit.id)
        ).all()

    @application.post("/api/v1/import-rows/{row_id}/review", tags=["Imports"])
    def review_import_row(row_id: UUID, payload: ImportRowReview, session: DatabaseSession):
        row = session.get(ImportRow, str(row_id))
        if row is None:
            raise HTTPException(status_code=404, detail="Import row not found")
        batch = session.get(ImportBatch, row.batch_id)
        account = reconciliation.lock_account(session, batch.account_id)
        row = session.get(ImportRow, str(row_id))
        if batch.status != "committed":
            raise HTTPException(
                status_code=409, detail="Only committed import decisions can be revised."
            )
        if row.classification in {"invalid", "excluded"} and payload.action != "skip":
            raise HTTPException(status_code=422, detail="This row cannot create a movement.")
        target = str(payload.transaction_id) if payload.transaction_id else None
        if payload.action == "link_duplicate":
            candidates = imports.match_candidates(session, batch, [row]).get(row.fingerprint, [])
            if target not in {item.id for item in candidates}:
                raise HTTPException(
                    status_code=422, detail="Target is not a matching transaction in this account."
                )
        previous_action, previous_target = row.decision, row.transaction_id
        if payload.action == "keep_separate":
            reusable = (
                row.transaction_id if row.decision in {"import_new", "keep_separate"} else None
            )
            if row.decision == "skip":
                last_audit = session.scalar(
                    select(ImportDecisionAudit)
                    .where(ImportDecisionAudit.row_id == row.id)
                    .order_by(ImportDecisionAudit.created_at.desc(), ImportDecisionAudit.id.desc())
                )
                if last_audit and last_audit.previous_action in {"import_new", "keep_separate"}:
                    reusable = last_audit.previous_transaction_id
            if reusable:
                target = reusable
            else:
                transaction = Transaction(
                    account_id=batch.account_id,
                    currency_code=row.currency_code,
                    product=row.product,
                    transaction_type=row.transaction_type,
                    posted_at=row.posted_at,
                    started_at=row.started_at,
                    amount=row.amount,
                    source_amount=row.source_amount,
                    fee=row.fee,
                    source_description=row.source_description,
                    fingerprint=row.fingerprint,
                )
                session.add(transaction)
                session.flush()
                target = transaction.id
        session.add(
            ImportDecisionAudit(
                row_id=row.id,
                previous_action=previous_action,
                previous_transaction_id=previous_target,
                action=payload.action,
                transaction_id=target,
                reason=payload.reason,
            )
        )
        row.decision = payload.action
        row.decided_at = utc_now()
        row.transaction_id = target if payload.action != "skip" else previous_target
        reconciliation.sync_periods(session, account)
        session.commit()
        return {"row_id": row.id, "decision": row.decision, "transaction_id": row.transaction_id}

    @application.post("/api/v1/import-rows/{row_id}/correct-time", tags=["Imports"])
    def correct_import_time(row_id: UUID, payload: ImportTimeCorrection, session: DatabaseSession):
        row = session.get(ImportRow, str(row_id))
        if row is None:
            raise HTTPException(status_code=404, detail="Import row not found")
        batch = session.get(ImportBatch, row.batch_id)
        account = reconciliation.lock_account(session, batch.account_id)
        row = session.get(ImportRow, str(row_id))
        if batch.status != "committed" or row.decision == "skip" or not row.transaction_id:
            raise HTTPException(
                status_code=409, detail="Only an accepted movement can be corrected."
            )
        linked_rows = session.scalars(
            select(ImportRow.id).where(
                ImportRow.transaction_id == row.transaction_id, ImportRow.decision != "skip"
            )
        ).all()
        if len(linked_rows) > 1:
            raise HTTPException(
                status_code=409,
                detail="Separate this linked source row before correcting its time.",
            )
        transaction = session.get(Transaction, row.transaction_id)
        local_time = payload.posted_at.replace(tzinfo=None)
        offset = payload.posted_at.strftime("%z")
        offset = f"{offset[:3]}:{offset[3:]}"
        session.add(
            ImportDecisionAudit(
                row_id=row.id,
                previous_action=row.decision,
                previous_transaction_id=row.transaction_id,
                action="time_correction",
                transaction_id=row.transaction_id,
                reason=payload.reason,
                previous_posted_at=transaction.posted_at,
                posted_at=local_time,
                previous_posted_offset=transaction.posted_offset,
                posted_offset=offset,
            )
        )
        row.corrected_posted_at = local_time
        row.corrected_offset = offset
        transaction.posted_at = local_time
        transaction.posted_offset = offset
        reconciliation.sync_periods(session, account)
        session.commit()
        return {
            "row_id": row.id,
            "source_posted_at": row.posted_at,
            "corrected_posted_at": row.corrected_posted_at,
            "corrected_offset": row.corrected_offset,
        }

    @application.get("/api/v1/imports/{preview_id}", tags=["Imports"])
    def read_import(preview_id: UUID, request: Request, session: DatabaseSession):
        return imports.read_preview(session, str(preview_id), import_owner(request))

    @application.post("/api/v1/imports/{preview_id}/commit", tags=["Imports"])
    def commit_import(
        preview_id: UUID,
        payload: ImportCommit,
        request: Request,
        session: DatabaseSession,
    ):
        return imports.commit_preview(
            session,
            str(preview_id),
            import_owner(request),
            [decision.model_dump() for decision in payload.decisions],
            payload.include_balance,
        )

    @application.post("/api/v1/imports/{preview_id}/cancel", tags=["Imports"])
    def cancel_import(preview_id: UUID, request: Request, session: DatabaseSession):
        return imports.cancel_preview(session, str(preview_id), import_owner(request))

    @application.get(
        "/api/v1/accounts/{account_id}/transactions",
        response_model=list[TransactionRead],
        tags=["Transactions"],
    )
    def list_transactions(
        account_id: UUID,
        session: DatabaseSession,
        limit: PageLimit = 100,
        offset: PageOffset = 0,
    ):
        get_account(session, account_id)
        transactions = session.scalars(
            select(Transaction)
            .where(Transaction.account_id == str(account_id))
            .where(
                or_(
                    Transaction.product == "Manual",
                    Transaction.id.in_(
                        select(ImportRow.transaction_id)
                        .where(ImportRow.transaction_id.is_not(None), ImportRow.decision != "skip")
                        .distinct()
                    ),
                )
            )
            .order_by(Transaction.posted_at.desc(), Transaction.id)
            .limit(limit)
            .offset(offset)
        ).all()
        provenance = {}
        if transactions:
            for row_id, transaction_id in session.execute(
                select(ImportRow.id, ImportRow.transaction_id).where(
                    ImportRow.transaction_id.in_([item.id for item in transactions]),
                    ImportRow.decision != "skip",
                )
            ):
                provenance.setdefault(transaction_id, []).append(row_id)
        result = []
        for transaction in transactions:
            item = TransactionRead.model_validate(transaction)
            item.import_row_ids = [UUID(row_id) for row_id in provenance.get(transaction.id, [])]
            manual_source = session.get(ManualTransactionSource, transaction.id)
            if manual_source:
                item.source = "manual"
                item.manual_reason = manual_source.reason
            result.append(item)
        return result

    return application


app = create_app()
