//! Defines the parser interface used by all statement parser crates.

use crate::{InputFormat, ParsedEntities, PipelineProfile};
use anyhow::Result;

/// Minimal contract for statement parser implementations.
///
/// Implementors parse one input file at a time and return normalized entities.
/// A parser executable can aggregate file-level outputs and then run a pipeline policy.
pub trait ParserContract {
    fn parser_name(&self) -> &'static str;

    fn supported_input_formats(&self) -> &'static [InputFormat];

    fn parse_file(&mut self, input_file_path: &str) -> Result<ParsedEntities>;

    fn finalize_entities(&mut self, entities: ParsedEntities) -> Result<ParsedEntities> {
        Ok(entities)
    }

    fn pipeline_profile(&self) -> PipelineProfile {
        PipelineProfile::MinimalImport
    }
}
