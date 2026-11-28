use super::{CASE_COUNT, fixture_root, read_inventory};
use std::{collections::BTreeSet, path::Path};

const PAINTING_COUNT: usize = 308;
const STRICT_PAINTING_COUNT: usize = 260;

#[test]
fn painting_cases_partition_the_source_tree_without_overlap() -> Result<(), String> {
    let inventory = read_inventory()?;
    let root = fixture_root();
    let mut all = BTreeSet::new();
    collect_painting(&root, &root.join("tests/painting"), &mut all)?;
    let fixed: BTreeSet<_> = inventory
        .cases
        .iter()
        .map(|case| case.input.clone())
        .collect();
    if fixed.len() != CASE_COUNT || all.len() != PAINTING_COUNT || !fixed.is_subset(&all) {
        return Err("painting source and fixed inventory differ".into());
    }
    let strict: BTreeSet<_> = all.difference(&fixed).cloned().collect();
    let combined: BTreeSet<_> = strict.union(&fixed).cloned().collect();
    if strict.len() != STRICT_PAINTING_COUNT || !strict.is_disjoint(&fixed) || combined != all {
        return Err("painting case partition differs".into());
    }
    Ok(())
}

fn collect_painting(
    root: &Path,
    directory: &Path,
    cases: &mut BTreeSet<String>,
) -> Result<(), String> {
    for entry in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        let path = entry.path();
        if kind.is_symlink() {
            return Err("painting source contains symlink".into());
        }
        if kind.is_dir() {
            collect_painting(root, &path, cases)?;
        } else if kind.is_file() && path.extension().is_some_and(|extension| extension == "svg") {
            add_painting(root, &path, cases)?;
        }
    }
    Ok(())
}

fn add_painting(root: &Path, path: &Path, cases: &mut BTreeSet<String>) -> Result<(), String> {
    let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
    let text = relative
        .to_str()
        .ok_or("non-UTF8 painting path")?
        .replace('\\', "/");
    if !cases.insert(text) {
        return Err("duplicate painting path".into());
    }
    Ok(())
}
