# Logger

Simple structured event logger for Matapan pipelines.

Writes append-only log lines for account/instrument/position/transaction additions, rule applications, and transaction removals.

Output path:

- default: `logs/YYYY-MM-DD.log` at the Matapan workspace root
- override with `MATAPAN_LOG_PATH`

Line format:

- `<timestamp-rfc3339> | <event_type> | <payload-json>`

The logger is intentionally best-effort and non-blocking for pipeline flow: logging failures are ignored.
