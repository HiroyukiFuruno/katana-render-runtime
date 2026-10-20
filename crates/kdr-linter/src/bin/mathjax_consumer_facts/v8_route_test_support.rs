use crate::source::{self, RUST_INPUTS, Source};
use std::collections::BTreeMap;

pub(super) type SourceRejection = (
    &'static str,
    usize,
    &'static str,
    fn(&syn::Block) -> Result<(), String>,
    &'static str,
);
pub(super) type ParsedRejection = (
    &'static str,
    usize,
    syn::Stmt,
    fn(&syn::Block) -> Result<(), String>,
    &'static str,
);

pub(super) fn parse_rejections(cases: &[SourceRejection]) -> Result<Vec<ParsedRejection>, String> {
    cases
        .iter()
        .map(|(name, index, statement, validate, expected)| {
            let statement = syn::parse_str(statement).map_err(|error| error.to_string())?;
            Ok((*name, *index, statement, *validate, *expected))
        })
        .collect()
}

pub(super) fn actual_sources() -> Result<BTreeMap<&'static str, Source>, String> {
    let root = crate::test_support::repository_root().map_err(|error| error.to_string())?;
    RUST_INPUTS
        .iter()
        .map(|path| Ok((*path, source::source(&root, path)?)))
        .collect()
}

pub(super) fn actual_evaluate_block() -> Result<syn::Block, String> {
    let sources = actual_sources()?;
    let runtime = sources
        .get(RUST_INPUTS[super::DIAGRAM_JS_RUNTIME_INPUT_INDEX])
        .ok_or("missing real V8 source")?;
    let evaluate = source::free_function(&runtime.ast, "evaluate")?;
    super::capture::evaluate(&evaluate.block)
}
