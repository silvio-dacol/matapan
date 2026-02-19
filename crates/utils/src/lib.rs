pub mod accounts;
pub mod database;
pub mod instruments;
pub mod positions;
pub mod transactions;

// Re-export commonly used items
pub use crate::accounts::{
    create_system_accounts, find_duplicate_account_ids, merge_accounts_with_deduplication,
};
pub use crate::database::{ensure_database_exists, read_database, write_database};
pub use crate::instruments::{
    find_duplicate_instrument_ids, merge_instruments_with_deduplication,
};
pub use crate::positions::{
    build_position, merge_positions_with_deduplication, normalize_position_pnl_fields,
    normalize_positions_pnl_fields, split_unrealized_pnl, PositionInput,
};
pub use crate::transactions::{
    find_duplicate_txn_ids, merge_transactions_with_deduplication, dedup_transactions_by_signature,
    mark_duplicates_by_signature, sort_transactions_by_date, MergeStats,
};
