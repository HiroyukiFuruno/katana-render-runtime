use resvg::usvg;
use std::{collections::BTreeSet, path::Path};

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
