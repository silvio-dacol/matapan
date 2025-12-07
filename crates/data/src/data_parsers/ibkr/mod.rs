use std::path::Path;

use crate::{ImportError, Importer, NormalizedTxn};

pub struct IbkrImporter;

impl Importer for IbkrImporter {
    fn name(&self) -> &'static str { "ibkr" }

    fn parse(&self, _path: &Path) -> Result<Vec<NormalizedTxn>, ImportError> {
        // Placeholder: implement parsing of IBKR reports
        Ok(Vec::new())
    }
}
