use std::path::Path;

use crate::{ImportError, Importer, NormalizedTxn};

pub struct RevolutImporter;

impl Importer for RevolutImporter {
    fn name(&self) -> &'static str { "revolut" }

    fn parse(&self, _path: &Path) -> Result<Vec<NormalizedTxn>, ImportError> {
        // Placeholder: implement CSV parsing of Revolut exports
        Ok(Vec::new())
    }
}
