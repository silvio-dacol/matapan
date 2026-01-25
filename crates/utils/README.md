# Utils

Shared utilities for reading/writing the database and transaction operations.

## AI: Translate Descriptions

This crate also provides a CLI tool to classify and (when needed) translate transaction descriptions into English.

It reads `database.json`, and for each unique transaction `description`:

- If it's already English or is a proper name / code that should not be translated: does nothing.
- Otherwise: writes an English translation to `description-en`.

### Prerequisites

- A local Ollama server must be running.
  - Desktop app is fine, or `ollama serve`.
  - Quick check: `Invoke-RestMethod http://localhost:11434/api/version`
- A model must be installed locally.
  - List models: `ollama list`
  - Pull a model if needed: `ollama pull qwen2.5:7b`

### Configuration

The CLI uses the local-only Ollama client from the `ai_client` crate.

Environment variables:

- `OLLAMA_BASE_URL` (default: `http://localhost:11434`)
- `OLLAMA_MODEL` (default: `llama3.2`, but must be installed locally)

### Usage

Dry-run (recommended first):

```powershell
$env:OLLAMA_MODEL='qwen2.5:7b'
cargo run -p utils --bin translate_descriptions -- --limit 10
```

Write changes back to `database.json`:

```powershell
$env:OLLAMA_MODEL='qwen2.5:7b'
cargo run -p utils --bin translate_descriptions -- --write
```

Other flags:

- `--db <path>`: database directory or `database.json` file (default: `./database`)
- `--force`: overwrite existing non-empty `description-en`
- `--limit N`: only process at most N unique descriptions

## Duplicate Handling

Two approaches are provided, both using a strict signature of core fields:

Signature: `(date, amount, currency, from_account_id, to_account_id, type)`

- Remove duplicates:
  - `dedup_transactions_by_signature(&mut db) -> Result<usize>`
  - Keeps the first occurrence and removes subsequent duplicates, preserving original order.
  - Returns the number of removed transactions.

- Mark duplicates:
  - `mark_duplicates_by_signature(&mut db) -> Result<usize>`
  - Keeps all transactions but annotates later duplicates with:
    - `duplicate: true`
    - `duplicate_of_txn_id: <first txn_id>` when available
  - Returns the number of marked transactions.

## Example

```rust
use anyhow::Result;
use serde_json::json;
use utils::{dedup_transactions_by_signature, mark_duplicates_by_signature};

fn main() -> Result<()> {
    let mut db = json!({
        "transactions": [
            {
                "date": "2025-01-01",
                "from_account_id": "A",
                "to_account_id": "B",
                "type": "internal_transfer",
                "amount": 100.0,
                "currency": "SEK",
                "txn_id": "X1"
            },
            {
                "date": "2025-01-01",
                "from_account_id": "A",
                "to_account_id": "B",
                "type": "internal_transfer",
                "amount": 100.0,
                "currency": "SEK",
                "txn_id": "X2"
            }
        ]
    });

    // Remove exact duplicates
    let removed = dedup_transactions_by_signature(&mut db)?;

    // Or mark duplicates instead
    let marked = mark_duplicates_by_signature(&mut db)?;
    Ok(())
}
```

## Notes

- These operations are conservative: differing `type` values (e.g., `income` vs `expense`) are not considered duplicates.
- For cross-source collapse (e.g., bank outflow + wallet inflow representing the same transfer), consider adding a separate normalization step with date tolerance and description normalization.
