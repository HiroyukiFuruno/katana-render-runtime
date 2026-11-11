use super::{assert_pixels_equal, parse_writer_svg, usvg};

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
