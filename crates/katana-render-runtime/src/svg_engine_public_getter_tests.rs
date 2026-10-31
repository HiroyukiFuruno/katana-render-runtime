use super::{upstream_writer_font_database, usvg};
use std::path::{Path, PathBuf};

#[path = "svg_engine_public_getter_text_parser_tests.rs"]
mod text_parser_tests;

#[test]
fn text_path_getters_match_authored_offset_and_path() -> Result<(), String> {
    let tree = parse_fixture("text/textPath/startOffset=30.svg")?;
    let text = text_node(&tree, "text1")?;
    let chunk = text.chunks().first().ok_or("text chunk missing")?;
    let path = match chunk.text_flow() {
        usvg::TextFlow::Path(path) => path,
        usvg::TextFlow::Linear => return Err("textPath flow missing".to_owned()),
    };
    assert_eq!(path.start_offset(), 30.0);
    assert!(path.path().segments().next().is_some());
    Ok(())
}

#[test]
fn text_span_length_adjust_matches_authored_value() -> Result<(), String> {
    let tree = parse_fixture("text/lengthAdjust/spacingAndGlyphs.svg")?;
    let text = text_node(&tree, "text1")?;
    let span = text
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .ok_or("text span missing")?;
    assert_eq!(span.length_adjust(), usvg::LengthAdjust::SpacingAndGlyphs);
    Ok(())
}

#[test]
fn embedded_webp_image_exposes_its_public_kind() -> Result<(), String> {
    let tree = parse_fixture("structure/image/embedded-webp.svg")?;
    let group = match tree.node_by_id("image1") {
        Some(usvg::Node::Group(group)) => group,
        _ => return Err("image1 wrapper group missing".to_owned()),
    };
    let mut images = Vec::new();
    collect_images(group, &mut images);
    assert_eq!(images.len(), 1, "image1 must contain exactly one Image");
    assert!(matches!(images[0].kind(), usvg::ImageKind::WEBP(_)));
    Ok(())
}

fn collect_images<'a>(group: &'a usvg::Group, images: &mut Vec<&'a usvg::Image>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect_images(group, images),
            usvg::Node::Image(image) => images.push(image),
            _ => {}
        }
    }
}

fn text_node<'a>(tree: &'a usvg::Tree, id: &str) -> Result<&'a usvg::Text, String> {
    match tree.node_by_id(id) {
        Some(usvg::Node::Text(text)) => Ok(text),
        _ => Err(format!("{id} node missing or not text")),
    }
}

fn parse_fixture(relative_path: &str) -> Result<usvg::Tree, String> {
    let path = fixture_path(relative_path);
    let bytes =
        std::fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        resources_dir: path.parent().map(Path::to_path_buf),
        ..usvg::Options::default()
    };
    usvg::Tree::from_data(&bytes, &options)
        .map_err(|error| format!("parse {}: {error}", path.display()))
}

fn fixture_path(relative_path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/svg-engine/upstream/resvg/tests/tests")
        .join(relative_path)
}
