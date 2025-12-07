pub mod importers;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("unsupported source: {0}")]
    UnsupportedSource(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),
    #[error("parse error: {0}")]
    Parse(String),
}

/// Minimal normalized record destined for the database JSON structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedTxn {
    pub date: String,
    pub account_id: String,
    pub r#type: String,
    pub category: Option<String>,
    pub amount: f64,
    pub currency: String,
    pub description: Option<String>,
}

/// Generic importer trait for bank/broker data sources.
pub trait Importer {
    fn name(&self) -> &'static str;
    /// Parse input file and return normalized transactions.
    fn parse(&self, path: &std::path::Path) -> Result<Vec<NormalizedTxn>, ImportError>;
}

/// Simple registry to pick importer by key.
pub fn get_importer(source: &str) -> Result<Box<dyn Importer>, ImportError> {
    match source.to_lowercase().as_str() {
        "revolut" => Ok(Box::new(importers::revolut::RevolutImporter)),
        "avanza" => Ok(Box::new(importers::avanza::AvanzaImporter)),
        "ibkr" => Ok(Box::new(importers::ibkr::IbkrImporter)),
        "nordnet" => Ok(Box::new(importers::nordnet::NordnetImporter)),
        _ => Err(ImportError::UnsupportedSource(source.to_string())),
    }
}
