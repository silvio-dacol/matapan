pub mod accounts;
pub mod contract;
pub mod database;
pub mod description_enrichment;
pub mod instruments;
pub mod round_digits;
pub mod pipeline;
pub mod positions;
pub mod rules;
pub mod transactions;

pub mod pipelines {
    pub use crate::pipeline::*;
}

pub mod descriptions {
    pub use crate::description_enrichment::*;
}

pub mod rule_sets {
    pub use crate::rules::*;
}

// Re-export commonly used items
pub use crate::accounts::{
    build_account, create_system_accounts, find_duplicate_account_ids,
    merge_accounts_with_deduplication, AccountInput,
};
pub use crate::contract::ParserContract;
pub use crate::database::{ensure_database_exists, read_database, write_database};
pub use crate::description_enrichment::{
    contains_non_latin_script, enrich_descriptions_to_english,
};
pub use crate::instruments::{
    build_instrument, find_duplicate_instrument_ids, merge_instruments_with_deduplication,
    InstrumentInput,
};
pub use crate::round_digits::{round_money, round_money_option};
pub use crate::pipeline::{
    discover_input_files, discover_input_files_in_current_dir, for_each_input_file,
    print_pipeline_summary, run_parser_contract_cli, run_parser_pipeline,
    run_parser_pipeline_with_policy, DedupStrategy, InputDiscovery, InputFormat, ParsedEntities,
    PipelineOptions, PipelinePolicy, PipelineProfile, PipelineSummary, PolicyEffects,
};
pub use crate::positions::{
    build_position, merge_positions_with_deduplication, normalize_position_pnl_fields,
    normalize_positions_pnl_fields, split_unrealized_pnl, PositionInput,
};
pub use crate::rules::{
    apply_rules, apply_rules_from_database_path, load_rules_from_database_path, Condition, Rule,
    RuleSet,
};
pub use crate::transactions::{
    build_transaction, dedup_transactions_by_date_amount_reference,
    dedup_transactions_by_date_and_amount, dedup_transactions_by_signature, find_duplicate_txn_ids,
    mark_duplicates_by_signature, merge_transactions_with_deduplication, sort_transactions_by_date,
    MergeStats, TransactionInput,
};
