#[test]
fn fallback_advance_rejects_nonfinite_coordinates() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(super::positions::advance_position(value).is_none());
    }
    assert_eq!(super::positions::advance_position(2.0), Some(2.0));
}

#[cfg(target_os = "macos")]
#[test]
fn corrupted_physical_fallback_font_rejects_glyph_cmap() -> Result<(), String> {
    let tree = tree_with_physical_noto("A日本", "", "Noto Sans", "Noto Sans")?;
    let prepared = prepare(&tree, text(&tree)?)?;
    let span = prepared
        .iter()
        .find(|span| span.glyphs.len() == 1 && span.glyphs[0].text == "A")
        .ok_or("Latin glyph run missing")?;
    assert!(super::eligibility::glyph_ids_match_cmap(
        tree.fontdb(),
        span.font,
        "A",
        &span.glyphs
    ));
    let face = tree.fontdb().face(span.font).ok_or("face missing")?.clone();
    assert_corrupt_font_cmap(face, &span.glyphs)
}

#[cfg(target_os = "macos")]
fn assert_corrupt_font_cmap(
    mut face: resvg::usvg::fontdb::FaceInfo,
    glyphs: &[resvg::usvg::layout::PositionedGlyph],
) -> Result<(), String> {
    use resvg::usvg::fontdb::{Database, Source};

    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("corrupted.ttf");
    write_corrupted_noto(&path)?;
    face.source = Source::File(path);
    let mut database = Database::new();
    let id = database.push_face_info(face);
    assert!(!super::eligibility::glyph_ids_match_cmap(
        &database, id, "A", glyphs
    ));
    Ok(())
}

#[cfg(target_os = "macos")]
fn write_corrupted_noto(path: &std::path::Path) -> Result<(), String> {
    let mut bytes = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf").to_vec();
    bytes[0] = !bytes[0];
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
#[test]
fn fallback_positions_reject_missing_or_extra_prepared_runs() -> Result<(), String> {
    let tree = tree("A日本", "")?;
    let text = text(&tree)?;
    let mut prepared = prepare(&tree, text)?;
    let source = super::eligibility::eligible_source(text, &prepared)
        .ok_or("fixture did not produce eligible fallback runs")?;
    assert!(super::positions::layout(tree.fontdb(), text, &prepared, source).is_some());
    let extra = prepare(&tree, text)?
        .into_iter()
        .next()
        .ok_or("fixture did not produce a prepared run")?;
    prepared.push(extra);
    let source = super::eligibility::eligible_source(text, &prepared[..prepared.len() - 1])
        .ok_or("fixture lost eligible fallback runs")?;
    assert!(super::positions::layout(tree.fontdb(), text, &prepared, source).is_none());
    prepared.pop().ok_or("extra run missing")?;
    prepared
        .pop()
        .ok_or("fixture did not produce enough runs")?;
    let source = super::sources::collect(text).ok_or("fixture source missing")?;
    assert!(super::positions::layout(tree.fontdb(), text, &prepared, source).is_none());
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn fallback_eligibility_rejects_incomplete_and_extra_runs() -> Result<(), String> {
    let tree = tree("A日本A", "")?;
    let text = text(&tree)?;
    let mut prepared = prepare(&tree, text)?;
    assert!(
        prepared.len() >= 3,
        "fixture must cross font boundaries twice"
    );
    assert!(super::eligibility::eligible_source(text, &prepared).is_some());
    assert!(super::eligibility::eligible_source(text, &[]).is_none());
    assert!(super::eligibility::eligible_source(text, &prepared[..prepared.len() - 1]).is_none());
    let extra = prepare(&tree, text)?
        .into_iter()
        .next()
        .ok_or("missing run")?;
    prepared.push(extra);
    assert!(super::eligibility::eligible_source(text, &prepared).is_none());
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn fallback_script_fragments_reject_inconsistent_run_data() -> Result<(), String> {
    let tree = tree("A日本", "")?;
    let text = text(&tree)?;
    let mut prepared = prepare(&tree, text)?;
    let source = &text
        .layouted()
        .first()
        .ok_or("missing source layout")?
        .positioned_glyphs;
    let scripts = super::resolved_scripts(source).ok_or("missing scripts")?;
    let span = prepared.first_mut().ok_or("missing prepared run")?;
    let run = source
        .get(..span.glyphs.len())
        .ok_or("run exceeds source")?;
    let run_scripts = scripts
        .get(..span.glyphs.len())
        .ok_or("run exceeds scripts")?;
    assert!(super::matching_script_fragments(run, span, run_scripts).is_some());
    assert!(
        super::matching_script_fragments(run, span, &run_scripts[..run_scripts.len() - 1])
            .is_none()
    );
    let glyph_id = span.glyph_ids.first_mut().ok_or("missing glyph ID")?;
    *glyph_id ^= 1;
    assert!(super::matching_script_fragments(run, span, run_scripts).is_none());
    Ok(())
}

#[cfg(target_os = "macos")]
use super::super::{HtmlGlyphRasterizer, Prepared, PreparedSpan};
#[cfg(target_os = "macos")]
use crate::markdown::svg_rasterize::font::html_font_db_for_text;
#[cfg(target_os = "macos")]
use resvg::usvg::{Node, Options, Tree, fontdb::Source};

#[cfg(target_os = "macos")]
const TITLE: &str = "インターネットバンキングシステム APIアプリケーションのコンポーネント図";
#[cfg(target_os = "macos")]
const FONT_FAMILIES: &str = "Trebuchet MS, Hiragino Kaku Gothic ProN";
#[cfg(target_os = "macos")]
const CSS_SCALE: &str = "0.9999806707719402";

#[cfg(target_os = "macos")]
use self::metrics_support::assert_run_layout_metrics;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_glyph_fallback_metrics_test_support.rs"]
mod metrics_support;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_glyph_fallback_capture_tests.rs"]
mod capture_tests;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_glyph_fallback_anchor_tests.rs"]
mod anchor_tests;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_glyph_fallback_diagnostic_tests.rs"]
mod diagnostic_tests;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_glyph_fallback_stock_paint_tests.rs"]
mod stock_paint_tests;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_glyph_fallback_script_tests.rs"]
mod script_tests;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_glyph_fallback_rejection_tests.rs"]
mod rejection_tests;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_glyph_fallback_multi_span_tests.rs"]
mod multi_span_tests;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_glyph_fallback_multi_chunk_tests.rs"]
mod multi_chunk_tests;

#[cfg(target_os = "macos")]
fn prepare(tree: &Tree, text: &resvg::usvg::Text) -> Result<Vec<PreparedSpan>, String> {
    let Prepared::Ready(prepared) = HtmlGlyphRasterizer::new(tree.fontdb()).prepare(text)? else {
        return Err("fallback fixture was not prepared".to_owned());
    };
    Ok(prepared)
}

#[cfg(target_os = "macos")]
fn tree(content: &str, attributes: &str) -> Result<Tree, String> {
    tree_with_fonts(content, attributes, "Trebuchet MS", FONT_FAMILIES)
}

#[cfg(target_os = "macos")]
fn tree_with_fonts(
    content: &str,
    attributes: &str,
    source_family: &str,
    database_families: &str,
) -> Result<Tree, String> {
    let database = html_font_db_for_text(database_families, content);
    tree_with_database(content, attributes, source_family, database)
}

#[cfg(target_os = "macos")]
fn tree_with_physical_noto(
    content: &str,
    attributes: &str,
    source_family: &str,
    database_families: &str,
) -> Result<Tree, String> {
    let mut database = html_font_db_for_text(database_families, content);
    let noto_faces = noto_face_ids(&database);
    if noto_faces.is_empty() {
        return Err("Noto Sans fixture face is missing".to_owned());
    }
    let database_mut = std::sync::Arc::make_mut(&mut database);
    for face in noto_faces {
        database_mut.remove_face(face);
    }
    load_physical_noto(database_mut)?;
    tree_with_database(content, attributes, source_family, database)
}

#[cfg(target_os = "macos")]
fn noto_face_ids(database: &resvg::usvg::fontdb::Database) -> Vec<resvg::usvg::fontdb::ID> {
    database
        .faces()
        .filter(|face| {
            face.families
                .iter()
                .any(|(family, _)| family.eq_ignore_ascii_case("Noto Sans"))
        })
        .map(|face| face.id)
        .collect()
}

#[cfg(target_os = "macos")]
fn load_physical_noto(database: &mut resvg::usvg::fontdb::Database) -> Result<(), String> {
    let font_path = physical_noto_path();
    database
        .load_font_file(&font_path)
        .map_err(|error| format!("physical Noto Sans fixture could not load: {error}"))?;
    if !database.faces().any(|face| {
        face.families
            .iter()
            .any(|(family, _)| family.eq_ignore_ascii_case("Noto Sans"))
            && matches!(&face.source, Source::File(path) if path == &font_path)
    }) {
        return Err("physical Noto Sans fixture was not registered".to_owned());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn physical_noto_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf")
}

#[cfg(target_os = "macos")]
fn tree_with_database(
    content: &str,
    attributes: &str,
    source_family: &str,
    database: std::sync::Arc<resvg::usvg::fontdb::Database>,
) -> Result<Tree, String> {
    let transform = if attributes.contains("transform=") {
        String::new()
    } else {
        format!("transform='scale({CSS_SCALE})'")
    };
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" x="216" y="20" font-family="{source_family}" font-size="16" {transform} {attributes}>{content}</text></svg>"#
    );
    Tree::from_str(
        &source,
        &Options {
            fontdb: database,
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn text(tree: &Tree) -> Result<&resvg::usvg::Text, String> {
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("fallback run text node is missing".to_owned());
    };
    Ok(text)
}
