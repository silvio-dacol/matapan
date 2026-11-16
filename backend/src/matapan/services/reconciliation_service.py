"""Timed cash references and reviewed, reversible synthetic adjustments.

All arithmetic uses canonical real movements once. Imported local timestamps are
interpreted only using a timezone explicitly supplied by the user. Calculations
are live proposals; an approved amount and its evidence are never silently edited.
"""

import hashlib
import json
from collections import defaultdict
from datetime import UTC, datetime, timedelta
from decimal import Decimal, localcontext
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

from sqlalchemy import or_, select, update
from sqlalchemy.orm import Session

from matapan.imports import ImportProblem
from matapan.models import (
    Account,
    AccountBalance,
    AppSetting,
    BalanceResolution,
    ImportBatch,
    ImportRow,
    Reconciliation,
    ReconciliationAdjustment,
    Transaction,
)


def timezone(name: str) -> ZoneInfo:
    try:
        return ZoneInfo(name)
    except (ZoneInfoNotFoundError, ValueError) as error:
        raise ImportProblem(
            422, "Use a valid IANA statement timezone, such as Europe/Zurich."
        ) from error


def effective_timezone(session: Session, account: Account) -> tuple[str | None, str]:
    """Resolve an account override before the Matapan-wide convention."""
    if account.statement_timezone:
        return account.statement_timezone, "account"
    setting = session.get(AppSetting, "default_statement_timezone")
    return (setting.value, "matapan") if setting else (None, "unset")


def lock_account(session: Session, account_id: str) -> Account:
    # Acquire SQLite's writer lock before taking a snapshot, just like import commit.
    session.execute(update(Account).where(Account.id == account_id).values(name=Account.name))
    account = session.get(Account, account_id)
    if account is None:
        raise ImportProblem(404, "Account not found")
    return account


def checkpoints(session: Session, account: Account) -> list[list[AccountBalance]]:
    grouped = defaultdict(list)
    for balance in session.scalars(
        select(AccountBalance)
        .where(
            AccountBalance.account_id == account.id,
            AccountBalance.currency_code == account.currency_code,
            AccountBalance.source == "manual",
            AccountBalance.status == "user_confirmed",
            AccountBalance.effective_at.is_not(None),
        )
        .order_by(AccountBalance.effective_at, AccountBalance.recorded_at, AccountBalance.id)
    ):
        grouped[balance.effective_at].append(balance)
    result = []
    for group in grouped.values():
        if len({item.amount for item in group}) > 1:
            from matapan.reports import balance_group_key

            key = balance_group_key(account.id, group)
            selection = session.scalar(
                select(BalanceResolution)
                .where(
                    BalanceResolution.account_id == account.id, BalanceResolution.group_key == key
                )
                .order_by(BalanceResolution.created_at.desc(), BalanceResolution.id.desc())
            )
            selected = next(
                (item for item in group if selection and item.id == selection.selected_balance_id),
                None,
            )
            if selected:
                group = [selected]
        result.append(group)
    return result


def current_pairs(session: Session, account: Account) -> set[tuple[str, str]]:
    groups = checkpoints(session, account)
    return {(left[0].id, right[0].id) for left, right in zip(groups, groups[1:], strict=False)}


def sync_periods(session: Session, account: Account) -> None:
    existing = set(
        session.execute(
            select(Reconciliation.opening_balance_id, Reconciliation.closing_balance_id).where(
                Reconciliation.account_id == account.id
            )
        ).all()
    )
    for opening, closing in current_pairs(session, account) - existing:
        session.add(
            Reconciliation(
                account_id=account.id,
                currency_code=account.currency_code,
                opening_balance_id=opening,
                closing_balance_id=closing,
            )
        )
    session.flush()


def checkpoint_view(balance: AccountBalance) -> dict:
    return {
        "id": balance.id,
        "amount": format(balance.amount, "f"),
        "effective_at": balance.effective_at.isoformat(),
        "effective_offset": balance.effective_offset,
    }


def source_instants(local: datetime, zone: ZoneInfo) -> set[datetime]:
    """Round-trip to reject nonexistent times and identify ambiguous DST folds."""
    candidates = set()
    for fold in (0, 1):
        instant = local.replace(tzinfo=zone, fold=fold).astimezone(UTC)
        if instant.astimezone(zone).replace(tzinfo=None) == local:
            candidates.add(instant)
    return candidates


def movements(
    session: Session,
    account: Account,
    start: datetime,
    end: datetime,
    zone_name: str | None,
) -> dict:
    result = {
        "movements": [],
        "issues": [],
        "imports": [],
        "excluded_rows": 0,
        "pending_rows": 0,
        "diagnostics": [
            {
                "code": "coverage_unverified",
                "message": (
                    "History coverage is unverified. A matching balance does not prove that "
                    "every transaction was imported."
                ),
                "action": "Use source statements and import history to review the interval.",
            }
        ],
    }
    rows = list(
        session.scalars(
            select(Transaction)
            .where(
                Transaction.account_id == account.id,
                Transaction.currency_code == account.currency_code,
                or_(
                    Transaction.product == "Manual",
                    Transaction.id.in_(
                        select(ImportRow.transaction_id)
                        .where(ImportRow.transaction_id.is_not(None), ImportRow.decision != "skip")
                        .distinct()
                    ),
                ),
            )
            .order_by(Transaction.posted_at, Transaction.id)
        )
    )
    source_rows = list(
        session.scalars(
            select(ImportRow)
            .join(ImportBatch, ImportBatch.id == ImportRow.batch_id)
            .where(ImportBatch.account_id == account.id, ImportBatch.status == "committed")
        )
    )
    needs_timezone = any(not row.posted_offset for row in rows) or any(
        not source.corrected_offset
        and (source.corrected_posted_at or source.posted_at or source.started_at)
        for source in source_rows
        if source.currency_code == account.currency_code
        and source.product == account.revolut_product
    )
    if not zone_name and (not rows or needs_timezone):
        result["issues"].append("Confirm the statement timezone before calculating movements.")
        result["diagnostics"].append(
            {
                "code": "timezone_required",
                "message": (
                    "Local statement timestamps need a timezone before Matapan can place "
                    "movements against timed checkpoints."
                ),
                "action": "Set Matapan's default timezone or an account-specific override.",
            }
        )
        return result
    zone = timezone(zone_name) if zone_name else None
    for row in rows:
        candidates = (
            {
                datetime.fromisoformat(
                    f"{row.posted_at.isoformat()}{row.posted_offset}"
                ).astimezone(UTC)
            }
            if row.posted_offset
            else source_instants(row.posted_at, zone)
        )
        relevant = [instant for instant in candidates if start < instant <= end]
        if len(candidates) != 1:
            if relevant or (
                not candidates
                and start.replace(tzinfo=None) - timedelta(days=1)
                <= row.posted_at
                <= end.replace(tzinfo=None) + timedelta(days=1)
            ):
                result["issues"].append(
                    f"Transaction {row.id} has ambiguous or nonexistent local time; "
                    "calculation is blocked."
                )
            continue
        if relevant:
            result["movements"].append(
                {
                    "id": row.id,
                    "amount": format(row.amount, "f"),
                    "posted_at": relevant[0].isoformat(),
                }
            )
    # Retain coverage and decision context without counting source occurrences as movements.
    batches = {}
    for source, batch in session.execute(
        select(ImportRow, ImportBatch)
        .join(ImportBatch, ImportBatch.id == ImportRow.batch_id)
        .where(ImportBatch.account_id == account.id, ImportBatch.status == "committed")
        .order_by(ImportRow.id)
    ):
        if (
            source.currency_code != account.currency_code
            or source.product != account.revolut_product
        ):
            continue
        local = source.corrected_posted_at or source.posted_at or source.started_at
        local_instants = (
            {
                datetime.fromisoformat(f"{local.isoformat()}{source.corrected_offset}").astimezone(
                    UTC
                )
            }
            if local is not None and source.corrected_offset
            else source_instants(local, zone)
            if local is not None
            else set()
        )
        if local is None or not any(start < value <= end for value in local_instants):
            continue
        result["excluded_rows"] += int(source.decision == "skip")
        result["pending_rows"] += int(source.source_state != "COMPLETED")
        if batch.id not in batches:
            batches[batch.id] = {
                "id": batch.id,
                "observed_start": batch.observed_start.isoformat()
                if batch.observed_start
                else None,
                "observed_end": batch.observed_end.isoformat() if batch.observed_end else None,
                "warnings": batch.warnings,
                "decisions": [],
            }
        batches[batch.id]["decisions"].append(
            {
                "row_id": source.id,
                "action": source.decision,
                "transaction_id": source.transaction_id,
            }
        )
    result["imports"] = list(batches.values())
    result["diagnostics"] = [
        {
            "code": "coverage_unverified",
            "message": (
                "The Revolut CSV has no declared statement period. Observed transaction dates "
                "show where rows exist, but cannot prove that all activity was imported."
            ),
            "action": (
                "Use source statements to review this interval; do not treat a zero difference "
                "as proof of complete history."
            ),
        }
    ]
    if not batches:
        result["diagnostics"].append(
            {
                "code": "no_import_rows",
                "message": (
                    "No matching imported source rows were observed inside this "
                    "checkpoint interval."
                ),
                "action": (
                    "Check import history and add any available statement covering this interval."
                ),
            }
        )
    if result["pending_rows"]:
        result["diagnostics"].append(
            {
                "code": "pending_rows",
                "message": (
                    f"{result['pending_rows']} pending or non-booked source row(s) fall in this "
                    "interval and were not counted as movements."
                ),
                "action": "Review the source rows and import a later booked statement if needed.",
            }
        )
    if result["excluded_rows"]:
        result["diagnostics"].append(
            {
                "code": "skipped_rows",
                "message": (
                    f"{result['excluded_rows']} source row(s) were skipped and were not counted "
                    "as movements."
                ),
                "action": "Review the row decision history if any skipped row should be included.",
            }
        )
    if any(
        "Running balances show a movement gap" in warning
        for batch in batches.values()
        for warning in batch["warnings"]
    ):
        result["diagnostics"].append(
            {
                "code": "running_balance_gap",
                "message": "A source running balance did not match the visible movement sequence.",
                "action": (
                    "Review source row ordering, duplicate decisions, and statement coverage "
                    "before resolving the difference."
                ),
            }
        )
    if batches and zone is not None:
        interval_start = start.astimezone(zone).date()
        interval_end = end.astimezone(zone).date()
        ranges = []
        for batch in batches.values():
            if not batch["observed_start"] or not batch["observed_end"]:
                continue
            first = datetime.fromisoformat(batch["observed_start"]).date()
            last = datetime.fromisoformat(batch["observed_end"]).date()
            first, last = max(first, interval_start), min(last, interval_end)
            if first <= last:
                ranges.append((first, last))
        ranges.sort()
        merged = []
        for first, last in ranges:
            if merged and first <= merged[-1][1] + timedelta(days=1):
                merged[-1] = (merged[-1][0], max(merged[-1][1], last))
            else:
                merged.append((first, last))
        for (_, previous_end), (next_start, _) in zip(merged, merged[1:], strict=False):
            gap_start, gap_end = previous_end + timedelta(days=1), next_start - timedelta(days=1)
            if gap_start <= gap_end:
                result["diagnostics"].append(
                    {
                        "code": "no_observed_activity",
                        "message": (
                            "No imported transaction rows were observed from "
                            f"{gap_start.isoformat()} "
                            f"through {gap_end.isoformat()} between the recorded activity ranges."
                        ),
                        "action": (
                            "This is an activity gap, not proof of missing transactions: the CSV "
                            "does not declare statement coverage. Check available source "
                            "statements."
                        ),
                    }
                )
    return result


def adjustments(session: Session, period_id: str) -> list[ReconciliationAdjustment]:
    return list(
        session.scalars(
            select(ReconciliationAdjustment)
            .where(
                ReconciliationAdjustment.reconciliation_id == period_id,
            )
            .order_by(ReconciliationAdjustment.approved_at, ReconciliationAdjustment.id)
        )
    )


def evaluate(session: Session, account: Account, period: Reconciliation) -> dict:
    opening = session.get(AccountBalance, period.opening_balance_id)
    closing = session.get(AccountBalance, period.closing_balance_id)
    current = (opening.id, closing.id) in current_pairs(session, account)
    zone_name, timezone_source = effective_timezone(session, account)
    evidence = movements(session, account, opening.effective_at, closing.effective_at, zone_name)
    issues = evidence["issues"]
    if not current:
        issues.append(
            "A new checkpoint split this interval. Retire its old ghost "
            "and review the current intervals."
        )
    endpoint_groups = [
        group
        for group in checkpoints(session, account)
        if group[0].effective_at in {opening.effective_at, closing.effective_at}
    ]
    if any(len({item.amount for item in group}) > 1 for group in endpoint_groups):
        issues.append(
            "Conflicting confirmations exist at a checkpoint; no adjustment can be approved."
        )
    # An old spanning ghost must be explicitly retired before accepting split intervals.
    for other in session.scalars(
        select(Reconciliation).where(
            Reconciliation.account_id == account.id,
            Reconciliation.id != period.id,
        )
    ):
        if (other.opening_balance_id, other.closing_balance_id) in current_pairs(session, account):
            continue
        other_open = session.get(AccountBalance, other.opening_balance_id)
        other_close = session.get(AccountBalance, other.closing_balance_id)
        if (
            other_open.effective_at < closing.effective_at
            and other_close.effective_at > opening.effective_at
        ):
            if any(item.state == "active" for item in adjustments(session, other.id)):
                issues.append(
                    "Retire the approved ghost for the previous overlapping interval first."
                )
    evidence.update(
        opening=checkpoint_view(opening),
        closing=checkpoint_view(closing),
        endpoint_observations=[
            [checkpoint_view(item) for item in group] for group in endpoint_groups
        ],
        statement_timezone=zone_name,
        statement_timezone_source=timezone_source,
        current=current,
    )
    history = adjustments(session, period.id)
    active = next((item for item in history if item.state == "active"), None)
    # Include approval history so concurrent/repeated approval cannot create a second ghost.
    signature = hashlib.sha256(
        json.dumps(
            {
                "evidence": evidence,
                "history_ids": [item.id for item in history],
            },
            sort_keys=True,
        ).encode()
    ).hexdigest()
    evidence_hash = hashlib.sha256(json.dumps(evidence, sort_keys=True).encode()).hexdigest()
    with localcontext() as context:
        context.prec = 64
        total = sum((Decimal(row["amount"]) for row in evidence["movements"]), Decimal(0))
        expected = opening.amount + total
        required = closing.amount - expected
        active_amount = active.amount if active else Decimal(0)
        difference = closing.amount - (expected + active_amount)
        adjusted_expected = expected + active_amount
    state = "reconciled" if difference == 0 else "unresolved"
    if history and history[-1].evidence_signature != evidence_hash:
        state = "needs_review"
    if issues:
        state = "blocked" if current else "superseded"
    diagnostics = list(evidence["diagnostics"])
    if issues:
        diagnostics.append(
            {
                "code": "calculation_blocked",
                "message": "Matapan could not safely calculate this checkpoint interval.",
                "action": (
                    "Resolve the listed timezone, checkpoint, or timestamp issue before "
                    "approving an adjustment."
                ),
            }
        )
    elif difference != 0:
        diagnostics.append(
            {
                "code": "unresolved_difference",
                "message": (
                    "The confirmed closing balance differs from the calculated balance by "
                    f"{format(difference, 'f')} {period.currency_code}."
                ),
                "action": (
                    "Investigate imports and row decisions, then confirm the proposed synthetic "
                    "adjustment only if appropriate."
                ),
            }
        )
    if state == "needs_review":
        diagnostics.append(
            {
                "code": "approval_needs_review",
                "message": (
                    "The evidence or timezone convention changed after the last adjustment "
                    "decision."
                ),
                "action": (
                    "Review the revised movements and amount before confirming or retiring the "
                    "prior adjustment."
                ),
            }
        )
    return {
        "id": period.id,
        "currency_code": period.currency_code,
        "opening": evidence["opening"],
        "closing": evidence["closing"],
        "state": state,
        "coverage": "unverified",
        "coverage_diagnostics": diagnostics,
        "current": current,
        "movement_count": len(evidence["movements"]),
        "real_total": format(total, "f"),
        "expected_close": format(adjusted_expected, "f") if not issues else None,
        "required_adjustment": format(required, "f") if not issues else None,
        "active_adjustment": format(active_amount, "f"),
        "difference": format(difference, "f") if not issues else None,
        "signature": signature,
        "evidence_hash": evidence_hash,
        "evidence": evidence,
        "issues": issues
        + ["Arithmetic agreement does not establish complete transaction history."],
        "history": [
            {
                "id": item.id,
                "amount": format(item.amount, "f"),
                "state": item.state,
                "action": item.action,
                "reason": item.reason,
                "approved_at": item.approved_at.isoformat(),
                "actor": item.actor,
                "replaces_id": item.replaces_id,
                "evidence": item.evidence,
            }
            for item in history
        ],
    }


def list_periods(session: Session, account: Account) -> list[dict]:
    return [
        evaluate(session, account, period)
        for period in session.scalars(
            select(Reconciliation)
            .where(Reconciliation.account_id == account.id)
            .order_by(Reconciliation.created_at, Reconciliation.id)
        )
    ]


def decide(session: Session, period_id: str, signature: str, reason: str, *, reverse=False) -> dict:
    # The first read obtains only the account identifier. End its snapshot before locking.
    account_id = session.scalar(
        select(Reconciliation.account_id).where(Reconciliation.id == period_id)
    )
    session.rollback()
    if account_id is None:
        raise ImportProblem(404, "Reconciliation not found")
    account = lock_account(session, account_id)
    period = session.get(Reconciliation, period_id)
    view = evaluate(session, account, period)
    if signature != view["signature"]:
        raise ImportProblem(
            409, "The evidence or approval changed. Refresh and review the new proposal."
        )
    if not reverse and view["state"] in {"blocked", "superseded"}:
        raise ImportProblem(409, "Resolve the interval issues before accepting an adjustment.")
    history = adjustments(session, period_id)
    active = next((item for item in history if item.state == "active"), None)
    amount = Decimal(0) if reverse else Decimal(view["required_adjustment"])
    if reverse and active is None:
        raise ImportProblem(409, "There is no active ghost to retire.")
    if active:
        active.state = "reversed" if amount == 0 else "superseded"
        session.flush()
    session.add(
        ReconciliationAdjustment(
            reconciliation_id=period_id,
            amount=amount,
            state="active" if amount != 0 else "reversed",
            action="reverse" if reverse or amount == 0 else "accept",
            reason=reason,
            replaces_id=history[-1].id if history else None,
            evidence_signature=view["evidence_hash"],
            evidence=view["evidence"],
        )
    )
    session.commit()
    return evaluate(session, account, period)


def projection(session: Session, account: Account, at: datetime) -> dict:
    at = at.astimezone(UTC)
    eligible = [group for group in checkpoints(session, account) if group[0].effective_at <= at]
    result = {
        "at": at.isoformat(),
        "amount": None,
        "status": "unavailable",
        "checkpoint": None,
        "movement_count": 0,
        "real_total": "0",
        "coverage": "unverified",
        "issues": [],
    }
    if not eligible:
        result["issues"] = [
            "No timed confirmed checkpoint exists on or before this instant. "
            "Balance is unknown, not zero."
        ]
        return result
    group = eligible[-1]
    checkpoint = group[0]
    result["checkpoint"] = checkpoint_view(checkpoint)
    if len({item.amount for item in group}) != 1:
        result["issues"] = ["Conflicting confirmations exist at this checkpoint."]
        return result
    if at == checkpoint.effective_at:
        result.update(amount=format(checkpoint.amount, "f"), status="confirmed")
        return result
    zone_name, _ = effective_timezone(session, account)
    evidence = movements(session, account, checkpoint.effective_at, at, zone_name)
    if evidence["issues"]:
        result["issues"] = evidence["issues"]
        return result
    with localcontext() as context:
        context.prec = 64
        total = sum((Decimal(row["amount"]) for row in evidence["movements"]), Decimal(0))
        result.update(
            amount=format(checkpoint.amount + total, "f"),
            status="calculated",
            real_total=format(total, "f"),
            movement_count=len(evidence["movements"]),
        )
    result["issues"] = [
        "Calculated from accepted movements after the checkpoint; "
        "transaction coverage is unverified.",
        "The confirmed checkpoint already includes earlier movements and adjustments.",
    ]
    return result
