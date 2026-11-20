use fontique::{Collection, CollectionOptions, SourceKind};
use std::{collections::BTreeSet, path::PathBuf};

pub(super) fn windows_system_font_paths(families: &BTreeSet<&str>) -> BTreeSet<PathBuf> {
    #[cfg(test)]
    let started = windows_font_trace_start();
    /* WHY: DirectWriteのファミリー索引を使い、指定された書体のファイルだけを描画DBへ渡す。 */
    let mut collection = Collection::new(CollectionOptions::default());
    let mut paths = BTreeSet::new();
    for family in families
        .iter()
        .filter_map(|name| collection.family_by_name(name))
    {
        for font in family.fonts() {
            if let SourceKind::Path(path) = font.source().kind() {
                paths.insert(path.to_path_buf());
            }
        }
    }
    #[cfg(test)]
    if let Some(started) = started {
        eprintln!(
            "[krr-html-trace] phase=windows_native_font_selection elapsed_us={} families={} paths={}",
            started.elapsed().as_micros(),
            families.len(),
            paths.len(),
        );
    }
    paths
}

#[cfg(test)]
fn windows_font_trace_start() -> Option<std::time::Instant> {
    std::env::var_os("DEBUG")
        .is_some_and(|value| value.to_string_lossy().eq_ignore_ascii_case("true"))
        .then(std::time::Instant::now)
}
