use super::font_file_fixture;
use super::{
    FIXTURE_HEIGHT, FIXTURE_WIDTH, HtmlGlyphRasterizer, assert_unsupported, parse_text,
    parse_text_content, render_bounds,
};
use resvg::usvg::{Node, Options, Tree};
use std::sync::Arc;
use tiny_skia::{Pixmap, Transform};

#[test]
fn removed_real_font_face_is_reported_unavailable() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("font.ttf");
    let fixture = font_file_fixture::create(path, font_file_fixture::FONT_BYTES)?;
    let mut database = fixture.database.clone();
    let face = fixture.face_id;
    let source = HtmlGlyphRasterizer::new(&database)
        .face_file(face)?
        .ok_or("file face missing")?;
    assert_eq!(source.0, fixture.path);
    assert!(fixture.path.is_file());
    database.remove_face(face);
    assert!(database.face(face).is_none());
    let error = HtmlGlyphRasterizer::new(&database)
        .face_file(face)
        .err()
        .ok_or("removed face was accepted")?;
    assert!(error.contains("SVG font face") && error.contains("is unavailable"));
    Ok(())
}

#[test]
fn unsupported_non_solid_fill_leaves_pixmap_unchanged() -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("krr-html-glyph-fill-{}.ttf", std::process::id()));
    let fixture = font_file_fixture::create(path, font_file_fixture::FONT_BYTES)
        .map_err(|error| error.to_string())?;
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="80"><text id="run" x="0" y="40" font-family="Noto Sans" font-size="28" fill="none">Ag</text></svg>"#;
    let tree = Tree::from_str(
        svg,
        &Options {
            fontdb: Arc::new(fixture.database.clone()),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("non-solid-fill text fixture missing".to_string());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let mut pixmap =
        Pixmap::new(FIXTURE_WIDTH, FIXTURE_HEIGHT).ok_or("pixmap allocation failed")?;
    assert_unsupported(&rasterizer, text, &mut pixmap)
}

#[test]
fn gradient_fill_falls_back_to_resvg_without_partial_glyph_paint() -> Result<(), String> {
    let path = std::env::temp_dir().join(format!(
        "krr-html-glyph-gradient-{}.ttf",
        std::process::id()
    ));
    let fixture = font_file_fixture::create(path, font_file_fixture::FONT_BYTES)
        .map_err(|error| error.to_string())?;
    let tree = gradient_tree(&fixture.database)?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("gradient text fixture missing".to_owned());
    };
    assert_gradient_fallback(&tree, text)
}

fn assert_gradient_fallback(tree: &Tree, text: &resvg::usvg::Text) -> Result<(), String> {
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let mut pixmap =
        Pixmap::new(FIXTURE_WIDTH, FIXTURE_HEIGHT).ok_or("pixmap allocation failed")?;
    assert_unsupported(&rasterizer, text, &mut pixmap)?;
    resvg::render(tree, tiny_skia::Transform::identity(), &mut pixmap.as_mut());
    assert!(
        pixmap
            .data()
            .chunks_exact(super::RGBA_CHANNELS)
            .any(|pixel| pixel[super::ALPHA_CHANNEL] != 0)
    );
    Ok(())
}

fn gradient_tree(database: &resvg::usvg::fontdb::Database) -> Result<Tree, String> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="80"><defs><linearGradient id="paint"><stop stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><text id="run" x="0" y="40" font-family="Noto Sans" font-size="28" fill="url(#paint)">Ag</text></svg>"##;
    Tree::from_str(
        svg,
        &Options {
            fontdb: Arc::new(database.clone()),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}

#[test]
fn malformed_typeface_is_rejected_without_poisoning_font_manager() -> Result<(), String> {
    let database = resvg::usvg::fontdb::Database::new();
    let rasterizer = HtmlGlyphRasterizer::new(&database);
    let malformed = skia_safe::Data::new_copy(b"not a font");
    assert!(rasterizer.create_backend_face(&malformed, 0)?.is_none());

    let valid = skia_safe::Data::new_copy(font_file_fixture::FONT_BYTES);
    assert!(rasterizer.create_backend_face(&valid, 0)?.is_some());
    Ok(())
}

const OUT_OF_RANGE_GLYPH: u32 = 70_000;

#[test]
fn glyph_outside_skia_id_range_rejects_the_run_without_partial_paint() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = font_file_fixture::create(directory.path().join("font.ttf"), &wide_cmap_font()?)
        .map_err(|error| error.to_string())?;
    let (tree, count) =
        parse_text_content(fixture.database.clone(), "Noto Sans", 400, false, "AB", "")?;
    assert_eq!(count, 2);
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("fixture text node missing".to_owned());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    assert!(rasterizer.ensure_face(fixture.face_id)?.is_some());
    assert!(
        text.layouted()
            .iter()
            .flat_map(|span| &span.positioned_glyphs)
            .any(|glyph| glyph.id.0 == OUT_OF_RANGE_GLYPH)
    );
    let mut pixmap =
        Pixmap::new(FIXTURE_WIDTH, FIXTURE_HEIGHT).ok_or("pixmap allocation failed")?;
    assert_unsupported(&rasterizer, text, &mut pixmap)
}

fn wide_cmap_font() -> Result<Vec<u8>, String> {
    use write_fonts::{
        FontBuilder,
        read::{FontRef, TableProvider},
        types::Tag,
    };
    let original =
        FontRef::new(font_file_fixture::FONT_BYTES).map_err(|error| error.to_string())?;
    let valid = original
        .cmap()
        .map_err(|error| error.to_string())?
        .map_codepoint('B')
        .ok_or("control glyph missing")?;
    let cmap = cmap_with_wide_glyph(valid.to_u32());
    let mut builder = FontBuilder::new();
    builder.add_raw(Tag::new(b"cmap"), &cmap);
    Ok(builder.copy_missing_tables(original).build())
}

fn cmap_with_wide_glyph(control: u32) -> Vec<u8> {
    const WINDOWS_PLATFORM_ID: u16 = 3;
    const FULL_UNICODE_ENCODING_ID: u16 = 10;
    const FORMAT_12: u16 = 12;
    const SUBTABLE_OFFSET: u32 = 12;
    const FORMAT_12_LENGTH: u32 = 40;
    /* WHY: Format 12の32ビットのglyph IDを使い、Skiaへ渡す直前の境界を検証する。 */
    let mut cmap = [0, 1, WINDOWS_PLATFORM_ID, FULL_UNICODE_ENCODING_ID]
        .into_iter()
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>();
    cmap.extend(SUBTABLE_OFFSET.to_be_bytes());
    cmap.extend([FORMAT_12, 0].into_iter().flat_map(u16::to_be_bytes));
    cmap.extend(
        [FORMAT_12_LENGTH, 0, 2]
            .into_iter()
            .flat_map(u32::to_be_bytes),
    );
    for (character, glyph) in [('A', OUT_OF_RANGE_GLYPH), ('B', control)] {
        let codepoint = u32::from(character);
        cmap.extend(
            [codepoint, codepoint, glyph]
                .into_iter()
                .flat_map(u32::to_be_bytes),
        );
    }
    cmap
}

#[test]
fn real_file_glyph_runs_reuse_cached_face_without_changing_pixels() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = font_file_fixture::create(
        directory.path().join("font.ttf"),
        font_file_fixture::FONT_BYTES,
    )
    .map_err(|error| error.to_string())?;
    for content in ["Ag", "VA", "A 9"] {
        let (tree, count) = parse_text_content(
            fixture.database.clone(),
            "Noto Sans",
            400,
            false,
            content,
            "",
        )?;
        assert!(count > 0);
        let Some(Node::Text(text)) = tree.node_by_id("run") else {
            return Err("glyph fixture text missing".to_owned());
        };
        let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
        assert_cached_glyph_pixels(&rasterizer, text, content)?;
    }
    Ok(())
}

fn assert_cached_glyph_pixels(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &resvg::usvg::Text,
    content: &str,
) -> Result<(), String> {
    let mut pixmap =
        Pixmap::new(FIXTURE_WIDTH, FIXTURE_HEIGHT).ok_or("pixmap allocation failed")?;
    let first = render_bounds(rasterizer, text, Transform::identity(), &mut pixmap)?;
    assert!(first.is_some(), "glyphs did not paint: {content}");
    let pixels = pixmap.data().to_vec();
    pixmap.fill(tiny_skia::Color::TRANSPARENT);
    let second = render_bounds(rasterizer, text, Transform::identity(), &mut pixmap)?;
    assert_eq!(second, first);
    assert_eq!(pixmap.data(), pixels.as_slice());
    assert_eq!(rasterizer.faces.borrow().len(), 1);
    Ok(())
}

#[test]
fn binary_font_fallback_renders_real_glyphs_without_partial_paint() -> Result<(), String> {
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_data(font_file_fixture::FONT_BYTES.to_vec());
    let (tree, count) = parse_text(database, "Noto Sans", 400, false)?;
    assert!(count > 0);
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("binary glyph fixture missing".to_owned());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let mut pixmap =
        Pixmap::new(FIXTURE_WIDTH, FIXTURE_HEIGHT).ok_or("pixmap allocation failed")?;
    assert_unsupported(&rasterizer, text, &mut pixmap)?;
    resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
    assert!(super::alpha_bounds(&pixmap).is_some());
    Ok(())
}
