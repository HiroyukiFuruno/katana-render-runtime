use super::{upstream_writer_font_database, usvg};
use std::path::{Path, PathBuf};

#[path = "svg_engine_public_getter_text_parser_tests.rs"]
mod text_parser_tests;

#[test]
fn empty_cdata_text_is_omitted_without_hiding_the_visible_sibling() -> Result<(), String> {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="40"><text id="empty"><![CDATA[]]></text><text id="visible" font-family="Noto Sans" y="20">Visible</text></svg>"#;
    let xml = usvg::roxmltree::Document::parse(source).map_err(|error| error.to_string())?;
    let empty = xml
        .root_element()
        .first_element_child()
        .ok_or("empty text element missing")?;
    assert!(
        empty
            .first_child()
            .is_some_and(|child| child.is_text() && child.text() == Some(""))
    );
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(source, &options).map_err(|error| error.to_string())?;
    assert!(tree.node_by_id("empty").is_none());
    let actual: String = text_node(&tree, "visible")?
        .chunks()
        .iter()
        .map(usvg::TextChunk::text)
        .collect();
    assert_eq!(actual, "Visible");
    Ok(())
}

#[test]
fn nested_group_uses_local_bounds_when_the_relative_transform_overflows() -> Result<(), String> {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><g id="outer" transform="scale(.001)"><g id="inner" transform="scale(1e20 1)"><rect width="1e19" height="2"/></g></g></svg>"#;
    let tree = usvg::Tree::from_str(source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let Some(usvg::Node::Group(outer)) = tree.node_by_id("outer") else {
        return Err("outer SVG group missing".into());
    };
    let Some(usvg::Node::Group(inner)) = tree.node_by_id("inner") else {
        return Err("inner SVG group missing".into());
    };
    let local = inner.bounding_box();
    assert!(local.transform(inner.transform()).is_none());
    assert_eq!(outer.bounding_box(), local);
    Ok(())
}

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

#[test]
fn fontdb_mut_clones_shared_database_before_loading_font() {
    let mut options = usvg::Options::default();
    let original = std::sync::Arc::clone(&options.fontdb);
    assert!(std::sync::Arc::ptr_eq(&original, &options.fontdb));
    options
        .fontdb_mut()
        .load_font_data(include_bytes!("../assets/fonts/NotoSans-Regular.ttf").to_vec());
    assert!(!std::sync::Arc::ptr_eq(&original, &options.fontdb));
    assert_eq!(original.faces().count(), 0);
    assert_eq!(options.fontdb.faces().count(), 1);
}
