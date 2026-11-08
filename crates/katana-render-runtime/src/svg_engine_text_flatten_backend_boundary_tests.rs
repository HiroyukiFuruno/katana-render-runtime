use super::{FONT, TestResult, usvg};
use crate::svg_engine::usvg::flatten::DatabaseExt as FlattenDatabaseExt;
use crate::svg_engine::usvg::layout::DatabaseExt as LayoutDatabaseExt;
use rstest::rstest;
use std::sync::Arc;

const SVG_FONT: &[u8] = include_bytes!(
    "../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/TwitterColorEmoji.subset.ttf"
);
const CBDT_FONT: &[u8] = include_bytes!(
    "../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/NotoColorEmojiCBDT.subset.ttf"
);
const SBIX_FONT: &[u8] = include_bytes!(
    "../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/CFF-and-SBIX.otf"
);
const COLR_FONT: &[u8] =
    include_bytes!("../../../tests/fixtures/svg-engine/colr-public-fonts/colr_1.ttf");
const COLR_GLYPH: u32 = 9;

#[derive(Clone, Copy)]
enum Backend {
    Outline,
    Raster,
    Svg,
    Colr,
}

#[derive(Clone, Copy)]
enum TestGlyph {
    Character(char),
    Id(u32),
}

fn fixture_database(bytes: &[u8]) -> TestResult<(Arc<usvg::fontdb::Database>, usvg::fontdb::ID)> {
    let mut database = usvg::fontdb::Database::new();
    let ids = database.load_font_source(usvg::fontdb::Source::Binary(Arc::new(bytes.to_vec())));
    let [face] = ids.as_slice() else {
        return Err("expected one fixture font face".into());
    };
    Ok((Arc::new(database), *face))
}

fn fixture_glyph(bytes: &[u8], glyph: TestGlyph) -> TestResult<usvg::GlyphId> {
    let id = match glyph {
        TestGlyph::Id(id) => id,
        TestGlyph::Character(character) => {
            let font = rustybuzz::ttf_parser::Face::parse(bytes, 0)
                .map_err(|error| format!("fixture font parsing failed: {error:?}"))?;
            u32::from(
                font.glyph_index(character)
                    .ok_or("fixture glyph missing")?
                    .0,
            )
        }
    };
    Ok(usvg::GlyphId(id))
}

fn backend_has_glyph(
    database: &usvg::fontdb::Database,
    face: usvg::fontdb::ID,
    glyph: usvg::GlyphId,
    backend: Backend,
) -> bool {
    match backend {
        Backend::Outline => FlattenDatabaseExt::outline(database, face, glyph, &[]).is_some(),
        Backend::Raster => FlattenDatabaseExt::raster(database, face, glyph).is_some(),
        Backend::Svg => FlattenDatabaseExt::svg(database, face, glyph).is_some(),
        Backend::Colr => FlattenDatabaseExt::colr(database, face, glyph, &[]).is_some(),
    }
}

#[rstest]
#[case::outline(FONT, TestGlyph::Character('A'), Backend::Outline)]
#[case::svg(SVG_FONT, TestGlyph::Character('😀'), Backend::Svg)]
#[case::cbdt(CBDT_FONT, TestGlyph::Character('😀'), Backend::Raster)]
#[case::sbix(SBIX_FONT, TestGlyph::Character('A'), Backend::Raster)]
#[case::colr(COLR_FONT, TestGlyph::Id(COLR_GLYPH), Backend::Colr)]
fn removed_face_is_rejected_by_font_and_flatten_backends(
    #[case] bytes: &[u8],
    #[case] test_glyph: TestGlyph,
    #[case] backend: Backend,
) -> TestResult<()> {
    let (mut database, face) = fixture_database(bytes)?;
    let glyph = fixture_glyph(bytes, test_glyph)?;
    assert!(LayoutDatabaseExt::load_font(database.as_ref(), face, &[]).is_some());
    assert!(backend_has_glyph(&database, face, glyph, backend));
    Arc::get_mut(&mut database)
        .ok_or("font database unexpectedly shared")?
        .remove_face(face);
    assert!(LayoutDatabaseExt::load_font(database.as_ref(), face, &[]).is_none());
    for backend in [
        Backend::Outline,
        Backend::Raster,
        Backend::Svg,
        Backend::Colr,
    ] {
        assert!(!backend_has_glyph(&database, face, glyph, backend));
    }
    Ok(())
}
