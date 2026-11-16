"""Fixture-driven tests for the verified English Revolut CSV variant."""

from decimal import Decimal

import pytest

from matapan.importers.parsers.revolut import MAX_FILE_BYTES, StatementError, parse_statement


def test_mixed_products_and_fees_are_parsed_without_merging(revolut_csv):
    statement = parse_statement(revolut_csv)
    assert len(statement.rows) == 6
    assert {row.product for row in statement.rows} == {"Current", "Savings"}
    assert {row.currency_code for row in statement.rows} == {"EUR", "SEK"}
    charged = next(row for row in statement.rows if row.fee != 0)
    assert charged.source_amount == Decimal("-10.00")
    assert charged.amount == Decimal("-11.25")
    assert charged.fee == Decimal("1.25")
    assert charged.posted_at.tzinfo is None
    assert all(not row.errors for row in statement.rows)


def test_optional_bom_and_decimal_precision(make_csv):
    source = make_csv({"Amount": "9007199254740993.12345678", "Fee": "0.00000001"})
    row = parse_statement(b"\xef\xbb\xbf" + source).rows[0]
    assert row.amount == Decimal("9007199254740993.12345677")


@pytest.mark.parametrize("source", [b"", b"Not,a,statement\n", b"\xff\xfe\x00", b"Type;Product\n"])
def test_unsupported_files_fail_without_guessing(source):
    with pytest.raises(StatementError):
        parse_statement(source)


def test_file_size_limit_is_enforced():
    with pytest.raises(StatementError, match="10 MiB"):
        parse_statement(b"x" * (MAX_FILE_BYTES + 1))


@pytest.mark.parametrize(
    "changes",
    [
        {"Completed Date": ""},
        {"Completed Date": "2025-02-30 10:00:00"},
        {"Amount": "NaN"},
        {"Fee": "-1.00"},
        {"Currency": "eu"},
        {"Product": "Unknown"},
    ],
)
def test_bad_rows_have_actionable_errors_without_private_values(make_csv, changes):
    source = make_csv({"Description": "PRIVATE SENTINEL", **changes})
    row = parse_statement(source).rows[0]
    assert row.errors
    assert row.fingerprint is None
    assert "PRIVATE SENTINEL" not in " ".join(row.errors)


def test_missing_balance_does_not_invent_a_total(make_csv):
    row = parse_statement(make_csv({"Balance": ""})).rows[0]
    assert row.balance is None
    assert not row.errors


def test_matching_identity_is_stable_but_is_not_a_provider_id(make_csv):
    first = parse_statement(
        make_csv({"Amount": "-10", "Description": "Synthetic   MERCHANT"})
    ).rows[0]
    second = parse_statement(
        make_csv({"Amount": "-10.00", "Description": "synthetic merchant"})
    ).rows[0]
    assert first.fingerprint == second.fingerprint
    assert first.source_description != second.source_description
