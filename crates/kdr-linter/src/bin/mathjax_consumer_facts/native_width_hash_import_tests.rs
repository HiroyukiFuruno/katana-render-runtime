use super::import_paths;
use std::collections::BTreeSet;

const RENAMED_IMPORT_SOURCE: &str = "use std::hash::{Hash as Digest, Hasher};";
const GLOB_IMPORT_SOURCE: &str = "use std::hash::*;";

#[test]
fn use_tree_rename_tracks_the_authored_import_path() -> Result<(), String> {
    let imports = imports_from(RENAMED_IMPORT_SOURCE)?;
    assert_eq!(
        imports,
        BTreeSet::from([
            "std::hash::Hash".to_string(),
            "std::hash::Hasher".to_string()
        ])
    );
    Ok(())
}

#[test]
fn use_tree_glob_does_not_claim_explicit_trait_imports() -> Result<(), String> {
    let imports = imports_from(GLOB_IMPORT_SOURCE)?;
    assert!(imports.is_empty());
    Ok(())
}

fn imports_from(source: &str) -> Result<BTreeSet<String>, String> {
    let file = syn::parse_file(source).map_err(|error| error.to_string())?;
    Ok(import_paths(&file))
}
