pub mod parser;
pub mod database;
pub mod pipeline;
pub mod description_enrichment;

pub mod accounts {
    pub use crate::parser::accounts::*;
}

pub mod instruments {
    pub use crate::parser::instruments::*;
}

pub mod positions {
    pub use crate::parser::positions::*;
}

pub mod transactions {
    pub use crate::parser::transactions::*;
}

pub mod pipelines {
    pub use crate::pipeline::*;
}

pub mod descriptions {
    pub use crate::description_enrichment::*;
}

// Re-export commonly used items
pub use crate::accounts::{
    build_account, create_system_accounts, find_duplicate_account_ids,
    merge_accounts_with_deduplication, AccountInput,
};
pub use crate::database::{ensure_database_exists, read_database, write_database};
pub use crate::description_enrichment::{
    contains_non_latin_script, enrich_descriptions_to_english,
};
pub use crate::pipeline::{
    apply_rules_from_database_path, discover_input_files, discover_input_files_in_current_dir,
    for_each_input_file, print_pipeline_summary, run_parser_pipeline, InputDiscovery,
    InputFormat, ParsedEntities, PipelineOptions, PipelineSummary,
};
pub use crate::instruments::{
    build_instrument, find_duplicate_instrument_ids, merge_instruments_with_deduplication,
    InstrumentInput,
};
pub use crate::positions::{
    build_position, merge_positions_with_deduplication, normalize_position_pnl_fields,
    normalize_positions_pnl_fields, split_unrealized_pnl, PositionInput,
};
pub use crate::transactions::{
    build_transaction, dedup_transactions_by_date_and_amount, dedup_transactions_by_signature,
    find_duplicate_txn_ids, mark_duplicates_by_signature, merge_transactions_with_deduplication,
    sort_transactions_by_date, MergeStats, TransactionInput,
};
