use std::path::Path;

use crate::{ImportError, Importer, NormalizedTxn};

pub struct AvanzaImporter;

impl Importer for AvanzaImporter {
    fn name(&self) -> &'static str { "avanza" }

    fn parse(&self, _path: &Path) -> Result<Vec<NormalizedTxn>, ImportError> {
        // Placeholder: implement CSV parsing of Avanza exports
        Ok(Vec::new())
    }
}
