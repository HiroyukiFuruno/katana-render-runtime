use super::super::super::BUNDLED_SANS_SERIF_FONT;
use resvg::usvg;
use std::{
    collections::BTreeSet,
    path::Path,
    sync::{Arc, OnceLock},
};

pub(super) fn bundled_font_source() -> usvg::fontdb::Source {
    static BUNDLED_FONT_DATA: OnceLock<Arc<dyn AsRef<[u8]> + Send + Sync>> = OnceLock::new();
    usvg::fontdb::Source::Binary(Arc::clone(
        BUNDLED_FONT_DATA.get_or_init(|| Arc::new(BUNDLED_SANS_SERIF_FONT.to_vec())),
    ))
}

#[cfg(test)]
pub(in super::super) fn load_font_paths(
    database: &mut usvg::fontdb::Database,
    paths: &[&str],
) -> usize {
    let mut loaded_paths = BTreeSet::new();
    load_font_paths_once(database, paths, &mut loaded_paths)
}

pub(in super::super) fn canonical_font_source_path(
    path: &Path,
) -> std::io::Result<std::path::PathBuf> {
    std::fs::canonicalize(path)
}

pub(in super::super) fn load_font_paths_once(
    database: &mut usvg::fontdb::Database,
    paths: &[&str],
    loaded_paths: &mut BTreeSet<std::path::PathBuf>,
) -> usize {
    let mut loaded = 0;
    for path in paths {
        let path = Path::new(path);
        let should_load = match canonical_font_source_path(path) {
            Ok(identity) => loaded_paths.insert(identity),
            Err(_) => true,
        };
        if path.is_file() && should_load {
            database.load_font_source(usvg::fontdb::Source::File(path.to_path_buf()));
            loaded += 1;
        }
    }
    loaded
}
