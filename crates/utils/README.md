# Utils

Shared utilities for reading/writing the database and transaction operations.

## Description Enrichment

Use `enrich_descriptions_to_english(&mut db)` to classify/translate transaction descriptions into `description-en`.

For each unique transaction `description`:

- If it's already English or should remain as-is (name/code), it is copied.
- Otherwise, it is translated to English.

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

Description enrichment and transaction sorting are now consumed as library functions
from parser pipelines (not as standalone `utils` binaries).

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
