# Utils

Shared utilities for reading/writing the database and transaction operations.

## Parser Contract

External parser crates should implement `utils::ParserContract`:

- `parser_name()`
- `supported_input_formats()`
- `parse_file(input_file_path)`
- Optional: `finalize_entities(...)`
- Optional: `pipeline_profile()`

The contract keeps parser crates focused on normalization, while the pipeline remains centralized.

## Pipeline Profiles

Use `run_parser_pipeline_with_policy(...)` with a profile-driven policy.

Available profiles:

- `PipelineProfile::RetailBankDefault`
  - Includes system accounts
  - Sorts by date
  - Applies `rules.json`
  - Enriches `description-en`
  - Deduplicates by date+amount
- `PipelineProfile::BrokerDefault`
  - Includes system accounts
  - Sorts by date
  - Applies `rules.json`
  - No description enrichment
  - No extra dedup pass (beyond `txn_id` merge dedup)
- `PipelineProfile::MinimalImport`
  - Includes system accounts only
  - No sorting/rules/description enrichment/extra dedup

Use profiles as defaults; parser CLIs can expose flags later to override policy fields.

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

## Rules Module

Rules are now exposed as library functions (similar to description enrichment), not as a standalone `utils` binary.

- Apply rules from `rules.json` located near the provided database path:

```rust
use utils::apply_rules_from_database_path;

let changed = apply_rules_from_database_path(&mut db, "./database")?;
```

- Reuse already-loaded rules directly:

```rust
use utils::{apply_rules, RuleSet};

let rules: RuleSet = serde_json::from_str(r#"{ "rules": [] }"#)?;
let changed = apply_rules(&mut db, &rules)?;
```

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
