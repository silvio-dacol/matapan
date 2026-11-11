"""Preserve explicit balance selections and import review corrections."""

import sqlalchemy as sa
from alembic import op

revision = "0004_review_decisions"
down_revision = "0003_reconciliation"
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.add_column("transactions", sa.Column("posted_offset", sa.String(6), nullable=True))
    op.add_column("import_rows", sa.Column("corrected_posted_at", sa.DateTime(), nullable=True))
    op.add_column("import_rows", sa.Column("corrected_offset", sa.String(6), nullable=True))
    op.create_table(
        "balance_resolutions",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("account_id", sa.String(36), nullable=False),
        sa.Column("group_key", sa.String(64), nullable=False),
        sa.Column("selected_balance_id", sa.String(36), nullable=False),
        sa.Column("reason", sa.String(500), nullable=False),
        sa.Column("created_at", sa.DateTime(), nullable=False),
        sa.ForeignKeyConstraint(["account_id"], ["accounts.id"], ondelete="RESTRICT"),
        sa.ForeignKeyConstraint(["selected_balance_id"], ["account_balances.id"]),
    )
    op.create_index(
        "ix_balance_resolution_group",
        "balance_resolutions",
        ["account_id", "group_key", "created_at"],
    )
    op.create_table(
        "import_decision_audit",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("row_id", sa.String(36), nullable=False),
        sa.Column("previous_action", sa.String(24), nullable=True),
        sa.Column("previous_transaction_id", sa.String(36), nullable=True),
        sa.Column("action", sa.String(24), nullable=False),
        sa.Column("transaction_id", sa.String(36), nullable=True),
        sa.Column("previous_posted_at", sa.DateTime(), nullable=True),
        sa.Column("posted_at", sa.DateTime(), nullable=True),
        sa.Column("previous_posted_offset", sa.String(6), nullable=True),
        sa.Column("posted_offset", sa.String(6), nullable=True),
        sa.Column("reason", sa.String(500), nullable=False),
        sa.Column("created_at", sa.DateTime(), nullable=False),
        sa.ForeignKeyConstraint(["row_id"], ["import_rows.id"], ondelete="RESTRICT"),
    )
    op.create_table(
        "manual_transaction_sources",
        sa.Column("transaction_id", sa.String(36), primary_key=True),
        sa.Column("reason", sa.String(500), nullable=False),
        sa.Column("created_at", sa.DateTime(), nullable=False),
        sa.ForeignKeyConstraint(["transaction_id"], ["transactions.id"], ondelete="RESTRICT"),
    )


def downgrade() -> None:
    raise RuntimeError("Preserve balance selections and import decision audit history.")
