# Utils

Shared utilities for reading/writing the database and transaction operations.

## Source Modules (very short)

- `src/accounts.rs`: account builders + account merge/dedup helpers.
- `src/contract.rs`: parser trait contract shared by parser crates.
- `src/instruments.rs`: instrument builders + instrument merge/dedup helpers.
- `src/positions.rs`: position builders + unrealized PnL normalization + merge.
- `src/transactions.rs`: transaction builders + sorting + merge/dedup helpers.
- `src/database.rs`: read/write/create helpers for `database.json`.
- `src/pipeline.rs`: shared parser pipeline orchestration and CLI runner.
- `src/description_enrichment.rs`: enriches `description-en` fields.
- `src/rules.rs`: loads/applies rule-based post-processing.

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

- `PipelineProfile::Default`
  - Includes system accounts
  - Sorts by date
  - Applies `rules.json`
  - Enriches `description-en`
  - No extra dedup (keeps parsed duplicates unless they were already skipped during merge)
- `PipelineProfile::StricterForDedup`
  - Includes system accounts
  - Sorts by date
  - Applies `rules.json`
  - Enriches `description-en`
  - Deduplicates by date+amount+reference (if present)
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

## Event Logging

Parser pipelines now emit structured event logs for:

- each account added,
- each instrument added,
- each position added,
- each transaction added,
- each rule application (with before/after snapshots),
- each transaction removed by dedup.

Events are appended to daily .log files:

- default: `logs/YYYY-MM-DD.log` at the Matapan workspace root
- override: `MATAPAN_LOG_PATH`

## Duplicate Handling

Current parser profiles use two duplicate layers:

- Merge-time dedup by `txn_id`:
  - `merge_transactions_with_deduplication(template, new_txns)`
  - Incoming rows with an already existing `txn_id` are skipped.

- Optional post-merge dedup by `date + amount + reference`:
  - `dedup_transactions_by_date_amount_reference(&mut db) -> Result<usize>`
  - Used by the `StricterForDedup` profile.
  - Reference lookup priority:
    1. `reference_number`
    2. `reference`
    3. `referens`
    4. token like `ref=...` in `description`
  - If no reference is found, fallback key is `date + amount`.
