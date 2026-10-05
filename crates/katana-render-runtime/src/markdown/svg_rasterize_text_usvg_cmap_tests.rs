use super::{
    has_char,
    sfnt::{CMAP_TAG, cmap_only_sfnt, has_character},
};
use resvg::usvg::fontdb::Database;
use skrifa::{FontRef, charmap::MappingIndex};

pub(super) const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

struct TempFont(std::path::PathBuf);

impl Drop for TempFont {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn cmap_only_wrapper_matches_skrifa_for_bundled_file_font() -> Result<(), String> {
    let font = FontRef::new(FONT_BYTES).map_err(|error| format!("font parse: {error}"))?;
    let cmap = font
        .table_data(CMAP_TAG)
        .ok_or("bundled cmap missing")?
        .as_bytes();
    let cmap_sfnt = cmap_only_sfnt(cmap).ok_or("cmap wrapper build failed")?;
    let mapping = MappingIndex::new(&font);
    for character in ['A', 'é', '中', '\u{10ffff}'] {
        let expected = skrifa::charmap::Charmap::new(&font)
            .map(character)
            .is_some();
        assert_eq!(
            has_character(&cmap_sfnt, mapping, character),
            Some(expected)
        );
    }
    Ok(())
}

#[test]
fn file_database_probe_matches_usvg_stock_for_bundled_font() -> Result<(), String> {
    let path = std::env::temp_dir().join(format!(
        "krr-usvg-cmap-{}-{}.ttf",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos(),
    ));
    std::fs::write(&path, FONT_BYTES).map_err(|error| error.to_string())?;
    let _guard = TempFont(path.clone());
    compare_file_probe(&path)
}

fn compare_file_probe(path: &std::path::Path) -> Result<(), String> {
    let mut database = Database::new();
    database
        .load_font_file(path)
        .map_err(|error| error.to_string())?;
    let database = std::sync::Arc::new(database);
    let face_id = database.faces().next().ok_or("font face missing")?.id;
    let durable = super::super::font::file_stamp_durable_reusable(path);
    for character in ['A', 'é', '中', '\u{10ffff}'] {
        let expected = stock_file_probe(&database, face_id, character)?;
        let expected_probe = durable.then_some(expected).ok_or(());
        let actual = super::super::with_validated_tree_parse(|| {
            (
                has_char(&database, face_id, character),
                has_char(&database, face_id, character),
            )
        });
        assert_eq!(actual, (expected_probe, expected_probe));
    }
    Ok(())
}

fn stock_file_probe(
    database: &Database,
    face_id: resvg::usvg::fontdb::ID,
    character: char,
) -> Result<bool, String> {
    Ok(database
        .with_face_data(face_id, |data, index| {
            let font = FontRef::from_index(data, index).ok()?;
            Some(
                skrifa::charmap::Charmap::new(&font)
                    .map(character)
                    .is_some(),
            )
        })
        .flatten()
        .ok_or("stock probe could not read font")?)
}

#[path = "svg_rasterize_text_usvg_cmap_file_tests.rs"]
mod file_tests;
#[path = "svg_rasterize_text_usvg_cmap_policy_tests.rs"]
mod policy_tests;

#[path = "svg_rasterize_text_usvg_cmap_ttc_tests.rs"]
mod ttc_tests;
