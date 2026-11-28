use super::{assert_pixels_equal, parse_writer_svg, usvg};

#[test]
fn preserved_text_path_with_extreme_finite_coordinates_keeps_its_definition() -> Result<(), String>
{
    let input = r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60">
      <defs><path id="extreme" d="M1.8e38 10 Q1.8e38 90 1.8e38 30"/>
        <path id="visible" d="M5 35 H110"/></defs>
      <text id="label" font-family="Noto Sans" font-size="12"><textPath href="#extreme">A</textPath><textPath href="#visible">Visible</textPath></text>
    </svg>"##;
    let case = "extreme finite text path";
    let original = parse_writer_svg(input, case)?;
    let Some(usvg::Node::Text(text)) = original.node_by_id("label") else {
        return Err("text path fixture was discarded".to_owned());
    };
    let paths: Vec<_> = text
        .chunks()
        .iter()
        .filter_map(|chunk| match chunk.text_flow() {
            usvg::TextFlow::Path(path) => Some(path),
            usvg::TextFlow::Linear => None,
        })
        .collect();
    assert_eq!(paths.len(), 2);
    assert!(paths[0].path().compute_tight_bounds().is_none());
    assert!(paths[1].path().compute_tight_bounds().is_some());
    assert_text_path_roundtrip(input, case, true)
}

#[test]
fn nested_group_text_path_definition_survives_writer_roundtrip() -> Result<(), String> {
    let input = r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60">
      <defs><path id="curve" d="M5 35 C30 10 70 10 110 35"/></defs>
      <g transform="translate(1 2)"><g opacity="0.9"><text font-family="Noto Sans" font-size="12">
      <textPath href="#curve" startOffset="10%">Nested path</textPath></text></g></g>
    </svg>"##;
    assert_text_path_roundtrip(input, "nested group text path", true)?;
    assert_text_path_roundtrip(input, "outlined nested group text path", false)
}

#[test]
fn mask_text_path_definition_survives_writer_roundtrip() -> Result<(), String> {
    let input = r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60">
      <defs><path id="curve" d="M5 35 C30 10 70 10 110 35"/>
      <mask id="letters" maskUnits="userSpaceOnUse" x="0" y="0" width="120" height="60">
      <text fill="white" font-family="Noto Sans" font-size="12"><textPath href="#curve">Masked</textPath></text>
      </mask></defs><rect width="120" height="60" fill="red" mask="url(#letters)"/>
    </svg>"##;
    assert_text_path_roundtrip(input, "mask text path", true)
}

fn assert_text_path_roundtrip(input: &str, case: &str, preserve_text: bool) -> Result<(), String> {
    let original = parse_writer_svg(input, case)?;
    let options = usvg::WriteOptions {
        preserve_text,
        ..usvg::WriteOptions::default()
    };
    let serialized = original.to_string(&options);
    if preserve_text {
        assert_text_path_definition(&serialized)?;
    } else {
        assert_outlined_text_path(&serialized)?;
    }
    let reparsed = parse_writer_svg(&serialized, case)?;
    let pixels = assert_pixels_equal(&original, &reparsed, case)?;
    assert!(pixels.pixels().iter().any(|pixel| pixel.alpha() > 0));
    Ok(())
}

fn assert_text_path_definition(serialized: &str) -> Result<(), String> {
    let document =
        usvg::roxmltree::Document::parse(serialized).map_err(|error| error.to_string())?;
    let text_path = document
        .descendants()
        .find(|node| node.has_tag_name("textPath"))
        .ok_or("serialized textPath missing")?;
    let reference = text_path
        .attribute(("http://www.w3.org/1999/xlink", "href"))
        .or_else(|| text_path.attribute("href"))
        .and_then(|href| href.strip_prefix('#'))
        .ok_or("textPath reference missing")?;
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("path") && node.attribute("id") == Some(reference))
    );
    Ok(())
}

fn assert_outlined_text_path(serialized: &str) -> Result<(), String> {
    let document =
        usvg::roxmltree::Document::parse(serialized).map_err(|error| error.to_string())?;
    assert!(!document.descendants().any(|node| node.has_tag_name("text")));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node
                .parent_element()
                .is_some_and(|parent| parent.has_tag_name("defs"))
    }));
    Ok(())
}
