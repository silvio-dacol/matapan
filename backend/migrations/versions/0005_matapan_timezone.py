"""Store Matapan-wide timezone conventions."""

import sqlalchemy as sa
from alembic import op

revision = "0005_matapan_timezone"
down_revision = "0004_review_decisions"
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.create_table(
        "app_settings",
        sa.Column("key", sa.String(80), primary_key=True),
        sa.Column("value", sa.String(200), nullable=False),
    )


def downgrade() -> None:
    raise RuntimeError("Preserve Matapan settings and migration history.")
