use super::{has_known_missing_base_table, usvg};
use crate::markdown::svg_rasterize::{font_file_fixture, html_glyph::HtmlGlyphRasterizer};
use read_fonts::{FontRef, TableProvider, types::Tag};
use rstest::rstest;
use std::sync::Arc;

const NOTO: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const CJK: &[u8] =
    include_bytes!("../../../../tests/fixtures/fonts/han-kerning/NotoSansCJKjp-han14.otf");

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
