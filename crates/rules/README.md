# Rules

A simple script that iterates over all transactions in `database/database.json` and applies straightforward, pattern-based rules.

## Usage

- Dry-run (no write):

```bash
cargo run -p rules
```

- Persist changes:

```bash
cargo run -p rules -- --write
```

- Specify database path and external rules file:

```bash
cargo run -p rules -- --db ./database --rules ./database/rules.json --write
```

## Rules Format (JSON)

Provide a file with the following shape:

```json
{
  "rules": [
    {
      "when": { "field": "description", "contains": "salary" },
      "set": { "category": "Income:Salary" }
    },
    {
      "when": { "field": "description", "contains": "amazon" },
      "set": { "category": "Shopping" }
    }
  ]
}
```

- `when.field`: the transaction field to inspect.
- `when.contains`: case-insensitive substring match on string fields.
- `when.equals`: exact JSON equality match (optional).
- `set`: key-value pairs to insert/update on the matched transaction.

If `--rules` is not provided, a small built-in ruleset is used.
