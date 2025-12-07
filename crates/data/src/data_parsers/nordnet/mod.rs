use std::path::Path;

use crate::{ImportError, Importer, NormalizedTxn};

pub struct NordnetImporter;

impl Importer for NordnetImporter {
    fn name(&self) -> &'static str { "nordnet" }

    fn parse(&self, _path: &Path) -> Result<Vec<NormalizedTxn>, ImportError> {
        // Placeholder: implement parsing of Nordnet exports
        Ok(Vec::new())
    }
}
