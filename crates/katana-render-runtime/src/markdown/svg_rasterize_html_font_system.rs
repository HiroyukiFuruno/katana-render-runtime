use resvg::usvg;
use std::{collections::BTreeSet, path::PathBuf};

pub(super) fn load_requested_system_font_family(
    database: &mut usvg::fontdb::Database,
    family: &str,
) {
    let mut system_database = usvg::fontdb::Database::new();
    system_database.load_system_fonts();
    let faces = system_database
        .faces()
        .map(|face| (face.families.as_slice(), &face.source));
    load_requested_font_paths(database, faces, family);
}

fn load_requested_font_paths<'a>(
    database: &mut usvg::fontdb::Database,
    faces: impl IntoIterator<
        Item = (
            &'a [(String, usvg::fontdb::Language)],
            &'a usvg::fontdb::Source,
        ),
    >,
    family: &str,
) {
    let paths = requested_font_paths(faces, family);
    for path in paths {
        database.load_font_source(usvg::fontdb::Source::File(path));
    }
}

fn requested_font_paths<'a>(
    faces: impl IntoIterator<
        Item = (
            &'a [(String, usvg::fontdb::Language)],
            &'a usvg::fontdb::Source,
        ),
    >,
    family: &str,
) -> BTreeSet<PathBuf> {
    faces
        .into_iter()
        .filter(|(families, _)| {
            families
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(family))
        })
        .filter_map(|(_, source)| source_path(source))
        .collect()
}

fn source_path(source: &usvg::fontdb::Source) -> Option<PathBuf> {
    if let usvg::fontdb::Source::File(path) = source {
        return Some(path.clone());
    }
    if let usvg::fontdb::Source::SharedFile(path, _) = source {
        return Some(path.clone());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        load_requested_font_paths, load_requested_system_font_family, requested_font_paths,
        source_path,
    };
    use resvg::usvg;
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn source_path_selects_only_file_backed_system_fonts() {
        let file_path = PathBuf::from("/tmp/krr-font.ttf");
        assert_eq!(
            source_path(&usvg::fontdb::Source::File(file_path.clone())),
            Some(file_path)
        );

        let shared_path = PathBuf::from("/tmp/krr-shared-font.ttf");
        let shared = usvg::fontdb::Source::SharedFile(shared_path.clone(), Arc::new(Vec::new()));
        assert_eq!(source_path(&shared), Some(shared_path));

        let binary = usvg::fontdb::Source::Binary(Arc::new(Vec::<u8>::new()));
        assert_eq!(source_path(std::hint::black_box(&binary)), None);
    }

    #[test]
    fn requested_font_paths_matches_family_and_keeps_only_file_sources() {
        let families = vec![(
            "KRR Synthetic Family".to_string(),
            usvg::fontdb::Language::English_UnitedStates,
        )];
        let file_path = PathBuf::from("/tmp/krr-font.ttf");
        let shared_path = PathBuf::from("/tmp/krr-shared-font.ttf");
        let file = usvg::fontdb::Source::File(file_path.clone());
        let shared = usvg::fontdb::Source::SharedFile(shared_path.clone(), Arc::new(Vec::new()));
        let binary = usvg::fontdb::Source::Binary(Arc::new(Vec::<u8>::new()));

        let paths = requested_font_paths(
            [
                (families.as_slice(), &file),
                (families.as_slice(), &shared),
                (families.as_slice(), &binary),
            ],
            "krr synthetic family",
        );

        assert_eq!(paths, [file_path, shared_path].into_iter().collect());
    }

    #[test]
    fn load_requested_font_paths_registers_matching_file_sources() {
        let families = vec![(
            "KRR Synthetic Family".to_string(),
            usvg::fontdb::Language::English_UnitedStates,
        )];
        let file = usvg::fontdb::Source::File(PathBuf::from("/tmp/krr-font.ttf"));
        let mut database = usvg::fontdb::Database::new();

        load_requested_font_paths(
            &mut database,
            [(families.as_slice(), &file)],
            "krr synthetic family",
        );
    }

    #[test]
    fn unknown_requested_family_scans_system_fonts_without_registering_all_faces() {
        let mut database = usvg::fontdb::Database::new();

        load_requested_system_font_family(&mut database, "krr-family-that-cannot-exist-7f0b");

        assert!(database.faces().next().is_none());
    }
}
