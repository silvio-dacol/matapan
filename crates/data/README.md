# data_import

Crate for importing financial data from various sources (banks, brokers) and normalizing it for the net-worth dashboard database format.

## Structure
- `src/lib.rs`: Public API, importer trait, normalized record types, registry.
- `src/importers/`: Per-provider modules
  - `revolut/`
  - `avanza/`
  - `ibkr/`
  - `nordnet/`

## Goals
- Parse CSV/Excel exports and convert to normalized transactions compatible with `database/template.json` structure.
- Keep each provider isolated under its own folder.

## Adding a new provider
1. Create `src/importers/<provider>/mod.rs`.
2. Implement `Importer` for `<Provider>Importer`.
3. Add `pub mod <provider>;` in `src/importers/mod.rs`.
4. Register in `get_importer` in `src/lib.rs`.

## Next steps
- Implement `parse` for Revolut/Avanza using `csv` crate.
- Define mapping to account and category IDs used in your database file.
