use super::{has_known_missing_base_table, usvg};
use crate::markdown::svg_rasterize::{font_file_fixture, html_glyph::HtmlGlyphRasterizer};
use rstest::rstest;
use skrifa::{
    MetadataProvider,
    raw::{FontRef, TableProvider, types::Tag},
};
use std::sync::Arc;

const NOTO: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const CJK: &[u8] =
    include_bytes!("../../../../tests/fixtures/fonts/han-kerning/NotoSansCJKjp-han14.otf");
const COLR_LINEAR_GLYPH: u32 = 9;

#[rstest]
#[case::noto_without_base(NOTO, false)]
#[case::cjk_with_base(CJK, true)]
fn known_missing_base_matches_real_font_table(
    #[case] bytes: &[u8],
    #[case] expected_base: bool,
) -> Result<(), String> {
    let font = FontRef::new(bytes).map_err(|error| format!("font parse failed: {error:?}"))?;
    let base_present = font.data_for_tag(Tag::new(b"BASE")).is_some();
    assert_eq!(base_present, expected_base);
    let mut database = usvg::fontdb::Database::new();
    let ids = database.load_font_source(usvg::fontdb::Source::Binary(Arc::new(bytes.to_vec())));
    let [id] = ids.as_slice() else {
        return Err("expected one font face".to_owned());
    };
    assert_eq!(has_known_missing_base_table(&database, *id), !base_present);
    Ok(())
}

#[test]
fn unknown_face_is_not_treated_as_known_missing_base() {
    let database = usvg::fontdb::Database::new();
    let unknown = usvg::fontdb::ID::dummy();
    assert!(database.face(unknown).is_none());
    assert!(!has_known_missing_base_table(&database, unknown));
}

#[test]
fn hanging_and_middle_reject_unavailable_primary_face() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = font_file_fixture::create(directory.path().join("NotoSans-Regular.ttf"), NOTO)
        .map_err(|error| error.to_string())?;
    for baseline in ["hanging", "middle"] {
        assert_primary_face_boundary(&fixture, baseline)?;
    }
    Ok(())
}

fn assert_primary_face_boundary(
    fixture: &font_file_fixture::FontFileFixture,
    baseline: &str,
) -> Result<(), String> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" x="0" y="20" dy="1" font-family="Noto Sans" dominant-baseline="{baseline}">AB</text></svg>"#
    );
    let tree = fixture.parse_svg(&svg).map_err(|error| error.to_string())?;
    let Some(usvg::Node::Text(text)) = tree.node_by_id("run") else {
        return Err("baseline fixture text missing".into());
    };
    assert!(super::uniform_policy_with_context(text, false, false).is_some());
    let empty = usvg::fontdb::Database::new();
    let missing = HtmlGlyphRasterizer::new(&empty);
    assert!(
        super::resolved_policy(&missing, text)?.is_none(),
        "{baseline}"
    );
    if baseline == "hanging" {
        let loaded = HtmlGlyphRasterizer::new(tree.fontdb());
        let policy =
            super::resolved_policy(&loaded, text)?.ok_or("loaded hanging policy missing")?;
        assert!(policy.primary_shift.is_some_and(f32::is_finite));
    }
    Ok(())
}

#[test]
fn middle_baseline_declines_unsupported_variable_color_face() -> Result<(), String> {
    let (tree, id) = variable_color_middle_tree()?;
    let Some(usvg::Node::Text(text)) = tree.node_by_id("run") else {
        return Err("color text missing".into());
    };
    assert!(super::uniform_policy_with_context(text, false, false).is_some());
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    assert!(rasterizer.face_metadata(id)?.unsupported);
    assert!(rasterizer.primary_variable_face(text, id)?.is_none());
    assert!(super::resolved_policy(&rasterizer, text)?.is_none());
    Ok(())
}

fn variable_color_middle_tree() -> Result<(usvg::Tree, usvg::fontdb::ID), String> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/svg-engine/colr-public-fonts/colr_1_variable.ttf");
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    let character = variable_color_character(&bytes)?;
    let mut database = usvg::fontdb::Database::new();
    let id = *database
        .load_font_source(usvg::fontdb::Source::File(path))
        .first()
        .ok_or("color face missing")?;
    let family = &database
        .face(id)
        .and_then(|face| face.families.first())
        .ok_or("color family missing")?
        .0;
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" x="0" y="20" dy="1" font-family="{family}" font-size="20" dominant-baseline="middle" font-variation-settings="'SWPS' 10">{character}</text></svg>"#
    );
    let tree = usvg::Tree::from_str(
        &svg,
        &usvg::Options {
            fontdb: Arc::new(database),
            ..usvg::Options::default()
        },
    )
    .map_err(|error| error.to_string())?;
    Ok((tree, id))
}

fn variable_color_character(bytes: &[u8]) -> Result<char, String> {
    let font = FontRef::new(bytes).map_err(|error| error.to_string())?;
    font.charmap()
        .mappings()
        .find_map(|(codepoint, glyph)| {
            (glyph.to_u32() == COLR_LINEAR_GLYPH)
                .then(|| char::from_u32(codepoint))
                .flatten()
        })
        .ok_or_else(|| "color font character missing".into())
}
