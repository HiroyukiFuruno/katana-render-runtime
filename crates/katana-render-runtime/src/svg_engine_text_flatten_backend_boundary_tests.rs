use super::{FONT, TestResult, font_fixture_tests::fixture_database, usvg};
use crate::svg_engine::usvg::flatten::DatabaseExt as FlattenDatabaseExt;
use crate::svg_engine::usvg::layout::DatabaseExt as LayoutDatabaseExt;
use rstest::rstest;
use std::sync::Arc;

const SVG_FONT: &[u8] = include_bytes!(
    "../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/TwitterColorEmoji.subset.ttf"
);
const SHARED_SVG_FONT: &[u8] = include_bytes!(
    "../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/NotoColorEmojiCOLR.subset.ttf"
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

#[test]
fn bitmap_text_in_clip_does_not_expand_the_vector_clip_region() -> TestResult<()> {
    let options = bitmap_options(CBDT_FONT)?;
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32">
      <defs><clipPath id="clip"><rect x="1" y="1" width="4" height="4"/>
      <text x="12" y="24" font-size="16">😀</text></clipPath></defs>
      <rect width="32" height="32" fill="blue" clip-path="url(#clip)"/></svg>"##;
    let tree = usvg::Tree::from_str(source, &options).map_err(|error| error.to_string())?;
    let clip = tree.clip_paths().first().ok_or("clip path missing")?;
    assert!(
        contains_bitmap(clip.root()),
        "clip fixture did not produce a bitmap"
    );
    let actual = render_clip_fixture(&tree)?;
    let expected = render_clip_fixture(
        &usvg::Tree::from_str(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32">
          <rect x="1" y="1" width="4" height="4" fill="blue"/></svg>"#,
            &options,
        )
        .map_err(|error| error.to_string())?,
    )?;
    assert_eq!(actual.data(), expected.data());
    assert_bitmap_clip_roundtrip(&tree, &options, &expected)
}

fn assert_bitmap_clip_roundtrip(
    tree: &usvg::Tree,
    parser: &usvg::Options<'_>,
    expected: &tiny_skia::Pixmap,
) -> TestResult<()> {
    for preserve_text in [false, true] {
        let serialized = tree.to_string(&usvg::WriteOptions {
            preserve_text,
            ..usvg::WriteOptions::default()
        });
        assert_eq!(serialized.contains('😀'), preserve_text);
        let reparsed =
            usvg::Tree::from_str(&serialized, parser).map_err(|error| error.to_string())?;
        assert_eq!(render_clip_fixture(&reparsed)?.data(), expected.data());
    }
    Ok(())
}

fn bitmap_options(bytes: &[u8]) -> TestResult<usvg::Options<'static>> {
    let (fontdb, face_id) = fixture_database(bytes)?;
    let family = fontdb
        .face(face_id)
        .and_then(|face| face.families.first())
        .ok_or("bitmap font family missing")?
        .0
        .clone();
    Ok(usvg::Options {
        fontdb,
        font_family: family,
        ..usvg::Options::default()
    })
}

#[path = "svg_engine_text_bitmap_mask_tests.rs"]
mod bitmap_mask_tests;

fn contains_bitmap(group: &usvg::Group) -> bool {
    group.children().iter().any(|node| match node {
        usvg::Node::Image(image) => matches!(image.kind(), usvg::ImageKind::PNG(_)),
        usvg::Node::Text(text) => contains_bitmap(text.flattened()),
        usvg::Node::Group(group) => contains_bitmap(group),
        usvg::Node::Path(_) => false,
    })
}

fn render_clip_fixture(tree: &usvg::Tree) -> TestResult<tiny_skia::Pixmap> {
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("pixmap missing")?;
    crate::svg_engine::resvg::render(tree, tiny_skia::Transform::identity(), &mut pixmap.as_mut());
    Ok(pixmap)
}

#[test]
fn shared_svg_document_selects_the_requested_glyph_node() -> TestResult<()> {
    let (database, face) = fixture_database(SHARED_SVG_FONT)?;
    assert!(FlattenDatabaseExt::svg(database.as_ref(), face, usvg::GlyphId(0)).is_none());
    for (character, expected_id) in [('🌓', "glyph1"), ('🥳', "glyph2")] {
        let glyph = fixture_glyph(SHARED_SVG_FONT, TestGlyph::Character(character))?;
        let node = FlattenDatabaseExt::svg(database.as_ref(), face, glyph)
            .ok_or("shared SVG glyph missing")?;
        assert_eq!(node.id(), expected_id);
    }
    Ok(())
}

#[test]
fn shared_svg_font_rejects_a_missing_glyph_id_and_malformed_document() -> TestResult<()> {
    const ORIGINAL_ID: &[u8] = br#"id="glyph1""#;
    const UNKNOWN_ID: &[u8] = br#"id="glyphX""#;
    let mut matches = SHARED_SVG_FONT
        .windows(ORIGINAL_ID.len())
        .enumerate()
        .filter(|(_, bytes)| *bytes == ORIGINAL_ID);
    let (id_offset, _) = matches.next().ok_or("fixture glyph1 marker missing")?;
    assert!(matches.next().is_none());
    let mut missing = SHARED_SVG_FONT.to_vec();
    missing[id_offset..id_offset + ORIGINAL_ID.len()].copy_from_slice(UNKNOWN_ID);
    assert_shared_svg_rejection(&missing, Some("glyph2"))?;
    let mut malformed = SHARED_SVG_FONT.to_vec();
    let root_offset = malformed[..id_offset]
        .windows(b"<svg".len())
        .rposition(|bytes| bytes == b"<svg")
        .ok_or("fixture SVG root missing")?;
    malformed[root_offset] = b'!';
    assert_shared_svg_rejection(&malformed, None)
}

#[test]
fn svg_in_ot_emoji_flattens_and_renders_through_the_text_route() -> TestResult<()> {
    let options = bitmap_options(SVG_FONT)?;
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
      <text id="emoji" x="4" y="48" font-size="40">😀</text>
    </svg>"#;
    let tree = usvg::Tree::from_str(source, &options)?;
    let Some(usvg::Node::Text(text)) = tree.node_by_id("emoji") else {
        return Err("SVG font text node missing".into());
    };
    let glyph = text
        .layouted()
        .iter()
        .flat_map(|span| span.positioned_glyphs.iter())
        .next()
        .ok_or("SVG font glyph was not shaped")?;
    assert!(FlattenDatabaseExt::svg(tree.fontdb().as_ref(), glyph.font, glyph.id).is_some());
    assert!(
        text.flattened()
            .children()
            .iter()
            .any(|node| matches!(node, usvg::Node::Group(_)))
    );

    let pixmap = render_clip_fixture(&tree)?;
    assert!(pixmap.pixels().iter().any(|pixel| pixel.alpha() != 0));
    Ok(())
}

fn assert_shared_svg_rejection(bytes: &[u8], sibling: Option<&str>) -> TestResult<()> {
    let (database, face) = fixture_database(bytes)?;
    let requested = fixture_glyph(bytes, TestGlyph::Character('🌓'))?;
    assert!(FlattenDatabaseExt::svg(database.as_ref(), face, requested).is_none());
    let other = fixture_glyph(bytes, TestGlyph::Character('🥳'))?;
    let node = FlattenDatabaseExt::svg(database.as_ref(), face, other);
    assert_eq!(node.as_ref().map(usvg::Node::id), sibling);
    Ok(())
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
