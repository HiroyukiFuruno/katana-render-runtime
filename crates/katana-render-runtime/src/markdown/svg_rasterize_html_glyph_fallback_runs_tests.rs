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
#[path = "svg_rasterize_html_glyph_fallback_anchor_tests.rs"]
mod anchor_tests;

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
