# Template parser scaffold (empty by design)

Updated by: GitHub Copilot
Last update: February 2026

This crate is a starting point for adding a new bank statement parser with the same architecture used by `revolut`.

Important: this template intentionally ships with no real parsing behavior. It compiles, but it returns empty entities until you implement the TODOs.

## What is intentionally empty

- `src/transactions.rs` returns `Ok((Vec::new(), Vec::new()))` and only contains TODO steps.
- `src/instruments.rs` and `src/positions.rs` are placeholder hooks.
- `src/accounts.rs` is a minimal example and should be replaced with your bank metadata.

## Source modules

- `src/main.rs`: CLI entrypoint using `utils::ParserContract`.
- `src/lib.rs`: parser facade + merge helpers exposed by the crate.
- `src/accounts.rs`: account mapping helpers.
- `src/transactions.rs`: commented TODO guide where you implement real transaction parsing.
- `src/instruments.rs`: placeholder hooks for investment instruments.
- `src/positions.rs`: placeholder hooks for investment positions.

## Next customization steps

1. Copy this crate into a new folder and rename package/parser identifiers (`template`, `TEMPLATE`).
2. Implement row deserialization + mapping in `src/transactions.rs` TODO blocks.
3. Replace account metadata in `src/accounts.rs`.
4. Add instrument/position parsing if your source supports investments.
5. Run `cargo check -p <your_new_parser_crate>`.
