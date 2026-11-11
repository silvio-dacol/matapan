"""Persistent previews, reviewed row matching, provenance, and atomic commits."""

import unicodedata
from collections import Counter, defaultdict
from datetime import timedelta
from decimal import localcontext

from sqlalchemy import select, update
from sqlalchemy.orm import Session

from matapan.db import utc_now
from matapan.importers.revolut import PARSER_ID, PARSER_VERSION, Statement, parse_statement
from matapan.models import (
    Account,
    AccountBalance,
    ImportBatch,
    ImportRow,
    ManualTransactionSource,
    Transaction,
)

PREVIEW_LIFETIME = timedelta(minutes=30)


class ImportProblem(Exception):
    def __init__(self, status_code: int, detail: str):
        super().__init__(detail)
        self.status_code = status_code
        self.detail = detail


def batch_rows(session: Session, batch_id: str) -> list[ImportRow]:
    return list(
        session.scalars(
            select(ImportRow).where(ImportRow.batch_id == batch_id).order_by(ImportRow.row_number)
        )
    )


def match_candidates(session: Session, batch: ImportBatch, rows: list[ImportRow]) -> dict:
    fingerprints = sorted({row.fingerprint for row in rows if row.fingerprint})
    matches = defaultdict(list)
    for start in range(0, len(fingerprints), 500):
        candidates = session.scalars(
            select(Transaction)
            .where(
                Transaction.account_id == batch.account_id,
                Transaction.currency_code == batch.currency_code,
                Transaction.product == batch.product,
                Transaction.fingerprint.in_(fingerprints[start : start + 500]),
            )
            .order_by(Transaction.id)
        )
        for candidate in candidates:
            matches[candidate.fingerprint].append(candidate)
    manual_rows = [
        row for row in rows if row.fingerprint and row.posted_at and row.amount is not None
    ]
    manual_candidates = (
        session.scalars(
            select(Transaction)
            .join(ManualTransactionSource, ManualTransactionSource.transaction_id == Transaction.id)
            .where(
                Transaction.account_id == batch.account_id,
                Transaction.currency_code == batch.currency_code,
                Transaction.product == batch.product,
                Transaction.amount.in_({row.amount for row in manual_rows}),
            )
        ).all()
        if manual_rows
        else []
    )
    for row in manual_rows:
        normalized = " ".join(
            unicodedata.normalize("NFKC", row.source_description).casefold().split()
        )
        for candidate in manual_candidates:
            candidate_description = " ".join(
                unicodedata.normalize("NFKC", candidate.source_description).casefold().split()
            )
            if (
                candidate.posted_at.date() == row.posted_at.date()
                and candidate.amount == row.amount
                and candidate_description == normalized
            ):
                matches[row.fingerprint].append(candidate)
    for key in matches:
        matches[key].sort(key=lambda item: item.id)
    return matches


def closing_row(rows: list[ImportRow]) -> ImportRow | None:
    available = [
        row
        for row in rows
        if row.classification not in {"invalid", "excluded"} and row.balance is not None
    ]
    if not available:
        return None
    latest = max(row.posted_at for row in available)
    at_latest = [row for row in available if row.posted_at == latest]
    if len({row.balance for row in at_latest}) != 1:
        return None  # Uncertain intraday ordering must not invent a closing checkpoint.
    return at_latest[-1]


def balance_preview(session: Session, batch: ImportBatch, rows: list[ImportRow]) -> dict | None:
    row = closing_row(rows)
    if row is None:
        return None
    checkpoint = row.posted_at.strftime("%Y-%m-%d %H:%M:%S")
    existing = session.scalar(
        select(AccountBalance).where(
            AccountBalance.account_id == batch.account_id,
            AccountBalance.currency_code == batch.currency_code,
            AccountBalance.source == "revolut",
            AccountBalance.source_timestamp == checkpoint,
        )
    )
    classification = "new"
    if existing:
        classification = "duplicate" if existing.amount == row.balance else "conflict"
    return {
        "classification": classification,
        "amount": format(row.balance, "f"),
        "as_of_date": row.posted_at.date().isoformat(),
        "source_timestamp": checkpoint,
        "row_id": row.id,
        "existing_balance_id": existing.id if existing else None,
        "source": "revolut",
        "status": "source_reported",
    }


def row_view(row: ImportRow, candidates: list[Transaction], finalized: bool) -> dict:
    classification = row.classification
    if not finalized and classification not in {"invalid", "excluded"}:
        classification = "possible_duplicate" if candidates else "new"
    return {
        "id": row.id,
        "row_number": row.row_number,
        "classification": classification,
        "product": row.product,
        "currency_code": row.currency_code,
        "transaction_type": row.transaction_type,
        "source_state": row.source_state,
        "description": row.source_description,
        "posted_at": row.posted_at,
        "started_at": row.started_at,
        "amount": format(row.amount, "f") if row.amount is not None else None,
        "source_amount": format(row.source_amount, "f") if row.source_amount is not None else None,
        "fee": format(row.fee, "f") if row.fee is not None else None,
        "errors": row.errors,
        "candidate_transactions": [
            {
                "id": candidate.id,
                "posted_at": candidate.posted_at,
                "amount": format(candidate.amount, "f"),
                "description": candidate.source_description,
            }
            for candidate in candidates
        ],
        "decision": row.decision,
        "transaction_id": row.transaction_id,
        "balance_id": row.balance_id,
    }


def preview_view(session: Session, batch: ImportBatch, *, rows_visible: bool = True) -> dict:
    rows = batch_rows(session, batch.id)
    matches = match_candidates(session, batch, rows) if rows_visible else {}
    finalized = batch.status == "committed"
    views = [row_view(row, matches.get(row.fingerprint, []), finalized) for row in rows]
    return {
        "id": batch.id,
        "account_id": batch.account_id,
        "currency_code": batch.currency_code,
        "product": batch.product,
        "parser_id": batch.parser_id,
        "parser_version": batch.parser_version,
        "status": batch.status,
        "already_imported": finalized,
        "expires_at": batch.expires_at,
        "observed_start": batch.observed_start,
        "observed_end": batch.observed_end,
        "declared_period": None,
        "counts": dict(Counter(view["classification"] for view in views)),
        "warnings": batch.warnings,
        "balance": balance_preview(session, batch, rows) if rows_visible else None,
        "rows": views if rows_visible else [],
    }


def initial_classification(row, account: Account) -> tuple[str, list[str]]:
    if row.product in {"Current", "Savings"} and row.product != account.revolut_product:
        return "excluded", ["This row belongs to the other product; use a separate account."]
    if row.currency_code and row.currency_code != account.currency_code:
        return "excluded", ["This row belongs to another currency; use a separate account."]
    if row.errors:
        return "invalid", list(row.errors)
    if row.source_state != "COMPLETED":
        return "excluded", ["Only COMPLETED rows are supported as booked movements."]
    return "new", []


def create_preview(session: Session, account_id: str, content: bytes, owner_hash: str) -> dict:
    statement: Statement = parse_statement(content)
    # First database statement is a write: serialize before taking a read snapshot.
    session.execute(update(Account).where(Account.id == account_id).values(name=Account.name))
    account = session.get(Account, account_id)
    if account is None:
        raise ImportProblem(404, "Account not found")
    if account.revolut_product not in {"Current", "Savings"}:
        raise ImportProblem(
            422, "Create the account with revolut_product set to Current or Savings."
        )
    if account.institution and "revolut" not in account.institution.casefold():
        raise ImportProblem(422, "This account is configured for a different institution.")
    existing = session.scalar(
        select(ImportBatch).where(
            ImportBatch.account_id == account.id,
            ImportBatch.file_hash == statement.file_hash,
            ImportBatch.parser_id == PARSER_ID,
            ImportBatch.parser_version == PARSER_VERSION,
        )
    )
    if existing:
        if existing.status == "committed":
            return preview_view(session, existing, rows_visible=existing.owner_hash == owner_hash)
        if existing.status == "preview" and existing.expires_at > utc_now():
            if existing.owner_hash != owner_hash:
                raise ImportProblem(
                    409, "This file has an active preview in another browser session."
                )
        else:
            # A fresh upload proves possession and renews an unconfirmed/cancelled plan.
            existing.status = "preview"
            existing.owner_hash = owner_hash
            existing.expires_at = utc_now() + PREVIEW_LIFETIME
        session.commit()
        return preview_view(session, existing)

    selected = [
        row
        for row in statement.rows
        if row.product == account.revolut_product and row.currency_code == account.currency_code
    ]
    if not selected:
        raise ImportProblem(422, "No rows match this account's product and currency.")
    batch = ImportBatch(
        account_id=account.id,
        currency_code=account.currency_code,
        product=account.revolut_product,
        file_hash=statement.file_hash,
        parser_id=PARSER_ID,
        parser_version=PARSER_VERSION,
        owner_hash=owner_hash,
        expires_at=utc_now() + PREVIEW_LIFETIME,
        warnings=[],
    )
    session.add(batch)
    session.flush()
    normalized = []
    for row in statement.rows:
        classification, errors = initial_classification(row, account)
        stored = ImportRow(
            batch_id=batch.id,
            row_number=row.row_number,
            classification=classification,
            errors=errors,
            product=row.product[:20],
            currency_code=row.currency_code[:3],
            source_state=row.source_state[:20],
            transaction_type=row.transaction_type[:64],
            source_description=row.source_description[:500],
            posted_at=row.posted_at,
            started_at=row.started_at,
            amount=row.amount,
            source_amount=row.source_amount,
            fee=row.fee,
            balance=row.balance,
            fingerprint=row.fingerprint,
        )
        session.add(stored)
        normalized.append(stored)
    session.flush()
    matches = match_candidates(session, batch, normalized)
    for row in normalized:
        if row.classification == "new" and matches.get(row.fingerprint):
            row.classification = "possible_duplicate"
    valid = [row for row in normalized if row.classification not in {"excluded", "invalid"}]
    if valid:
        batch.observed_start = min(row.posted_at for row in valid)
        batch.observed_end = max(row.posted_at for row in valid)
    warnings = [
        "This file has no trusted transaction IDs; matching fields require duplicate review.",
        "Observed transaction dates are not a declared statement period or proof of full coverage.",
        "Source timestamps have no timezone; they are retained as local provider timestamps.",
        "Normalized preview rows are kept in local SQLite; original CSV bytes are not retained.",
    ]
    if any(row.classification == "excluded" for row in normalized):
        warnings.append("Other products, currencies, or non-completed rows are excluded.")
    if any(row.classification == "invalid" for row in normalized):
        warnings.append("Invalid rows must be explicitly skipped before commit.")
    if any(count > 1 for count in Counter(row.fingerprint for row in valid).values()):
        warnings.append("Equal matching fields occur within this file; new rows remain separate.")
    if any(row.balance is None for row in valid) or closing_row(normalized) is None:
        warnings.append("A latest closing balance is not established; no balance will be invented.")
    with localcontext() as context:
        context.prec = 64
        for previous, current in zip(valid, valid[1:], strict=False):
            if (
                previous.balance is not None
                and current.balance is not None
                and current.balance - previous.balance != current.amount
            ):
                warnings.append("Running balances show a movement gap or ordering uncertainty.")
                break
    batch.warnings = warnings
    session.commit()
    return preview_view(session, batch)


def owned_batch(session: Session, batch_id: str, owner_hash: str) -> ImportBatch:
    batch = session.get(ImportBatch, batch_id)
    if batch is None or batch.owner_hash != owner_hash:
        raise ImportProblem(404, "Import preview not found in this browser session")
    if batch.status == "cancelled":
        raise ImportProblem(409, "This preview was cancelled; upload the file again to start over.")
    if batch.status == "preview" and batch.expires_at <= utc_now():
        raise ImportProblem(410, "This preview expired; upload the file again.")
    return batch


def read_preview(session: Session, batch_id: str, owner_hash: str) -> dict:
    return preview_view(session, owned_batch(session, batch_id, owner_hash))


def cancel_preview(session: Session, batch_id: str, owner_hash: str) -> dict:
    session.execute(update(ImportBatch).where(ImportBatch.id == batch_id).values(id=ImportBatch.id))
    batch = owned_batch(session, batch_id, owner_hash)
    if batch.status == "committed":
        raise ImportProblem(409, "A committed import cannot be cancelled.")
    batch.status = "cancelled"
    session.commit()
    return {"id": batch.id, "status": "cancelled"}


def commit_result(batch: ImportBatch, rows: list[ImportRow], *, new: int = 0) -> dict:
    return {
        "id": batch.id,
        "status": batch.status,
        "new_transactions": new,
        "transactions_in_batch": len({row.transaction_id for row in rows if row.transaction_id}),
        "linked_duplicates": sum(row.decision == "link_duplicate" for row in rows),
        "skipped_rows": sum(row.decision == "skip" for row in rows),
        "balance_id": next((row.balance_id for row in rows if row.balance_id), None),
    }


def commit_preview(
    session: Session, batch_id: str, owner_hash: str, decisions: list[dict], include_balance: bool
) -> dict:
    # Acquire SQLite's writer lock before reads; concurrent commits then re-read fresh state.
    session.execute(update(ImportBatch).where(ImportBatch.id == batch_id).values(id=ImportBatch.id))
    batch = owned_batch(session, batch_id, owner_hash)
    rows = batch_rows(session, batch.id)
    if batch.status == "committed":
        result = commit_result(batch, rows)
        result["already_imported"] = True
        return result
    by_id = {str(decision["row_id"]): decision for decision in decisions}
    if len(by_id) != len(decisions) or not by_id.keys() <= {row.id for row in rows}:
        raise ImportProblem(422, "Decisions contain repeated or unknown row IDs.")
    matches = match_candidates(session, batch, rows)
    plans = []
    for row in rows:
        decision = by_id.get(row.id)
        action = decision["action"] if decision else None
        target = (
            str(decision["transaction_id"]) if decision and decision.get("transaction_id") else None
        )
        if row.classification == "excluded":
            if action not in {None, "skip"}:
                raise ImportProblem(422, "Excluded rows cannot create or link financial movements.")
            plans.append((row, "skip", None))
            continue
        if row.classification == "invalid":
            if action != "skip":
                raise ImportProblem(409, f"Row {row.row_number} is invalid; explicitly skip it.")
            plans.append((row, "skip", None))
            continue
        candidates = matches.get(row.fingerprint, [])
        if action is None:
            if candidates:
                raise ImportProblem(
                    409, f"Row {row.row_number} requires duplicate review; refresh the preview."
                )
            action = "import_new"
        if action == "link_duplicate" and target not in {candidate.id for candidate in candidates}:
            raise ImportProblem(
                422, "Duplicate target is not a matching transaction in this account."
            )
        if action not in {"import_new", "keep_separate", "link_duplicate", "skip"}:
            raise ImportProblem(422, "Unsupported row decision")
        plans.append((row, action, target))
    balance = balance_preview(session, batch, rows) if include_balance else None
    if balance and balance["classification"] == "conflict":
        raise ImportProblem(
            409, "A conflicting source balance exists; review it or set include_balance=false."
        )

    added = 0
    for row, action, target in plans:
        row.decision = action
        row.decided_at = utc_now()
        if action in {"import_new", "keep_separate"}:
            transaction = Transaction(
                account_id=batch.account_id,
                currency_code=batch.currency_code,
                product=batch.product,
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
            row.transaction_id = transaction.id
            added += 1
        elif action == "link_duplicate":
            row.transaction_id = target
    if balance:
        row = next(row for row in rows if row.id == balance["row_id"])
        if balance["existing_balance_id"]:
            row.balance_id = balance["existing_balance_id"]
        else:
            observation = AccountBalance(
                account_id=batch.account_id,
                currency_code=batch.currency_code,
                as_of_date=row.posted_at.date(),
                amount=row.balance,
                source="revolut",
                status="source_reported",
                source_timestamp=balance["source_timestamp"],
            )
            session.add(observation)
            session.flush()
            row.balance_id = observation.id
    batch.status = "committed"
    batch.committed_at = utc_now()
    session.commit()
    result = commit_result(batch, rows, new=added)
    result["already_imported"] = False
    return result
