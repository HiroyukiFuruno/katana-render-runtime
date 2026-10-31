use super::{AId, Document};

fn parse_document<'a>(source: &'a str, injected: Option<&'a str>) -> Result<Document<'a>, String> {
    let xml = roxmltree::Document::parse(source).map_err(|error| error.to_string())?;
    Document::parse_tree(&xml, injected).map_err(|error| error.to_string())
}

fn raw_attribute<'doc, 'input>(
    document: &'doc Document<'input>,
    id: &str,
    aid: AId,
) -> Result<&'doc str, String> {
    document
        .element_by_id(id)
        .ok_or_else(|| format!("element {id} missing"))?
        .attributes()
        .iter()
        .find(|attribute| attribute.name == aid)
        .map(|attribute| attribute.value.as_str())
        .ok_or_else(|| format!("attribute {aid:?} missing on {id}"))
}

fn assert_one_neutralized_link(
    document: &Document<'_>,
    ids: [&str; 2],
    aid: AId,
) -> Result<(), String> {
    let values = [
        raw_attribute(document, ids[0], aid)?,
        raw_attribute(document, ids[1], aid)?,
    ];
    if values.iter().filter(|value| **value == "none").count() != 1
        || values
            .iter()
            .filter(|value| value.starts_with("url(#"))
            .count()
            != 1
    {
        return Err(format!(
            "recursive links on {ids:?} were not reduced to one edge"
        ));
    }
    Ok(())
}

fn assert_attribute_value(source: &str, id: &str, aid: AId, expected: &str) -> Result<(), String> {
    let document = parse_document(source, None)?;
    if raw_attribute(&document, id, aid)? != expected {
        return Err(format!(
            "{aid:?} on {id} did not match the authored contract"
        ));
    }
    Ok(())
}

fn assert_recursive_pair(source: &str, ids: [&str; 2], aid: AId) -> Result<(), String> {
    let document = parse_document(source, None)?;
    assert_one_neutralized_link(&document, ids, aid)
}

#[test]
fn css_cascade_inline_style_first_child_and_unsupported_pseudo_are_distinguished()
-> Result<(), String> {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg">
      <style>.tone { fill: red; } .tone { fill: blue; }
      rect:first-child { stroke: green; } rect:hover { stroke: red; }</style>
      <style/><g><rect id="first" class="tone" fill="black" style="fill: yellow"/>
      <rect id="second" class="tone"/></g></svg>"#;
    let document = parse_document(source, Some(".tone { fill: lime; }"))?;
    let first = document
        .element_by_id("first")
        .ok_or_else(|| "first rect missing".to_owned())?;
    let second = document
        .element_by_id("second")
        .ok_or_else(|| "second rect missing".to_owned())?;

    if first.attribute::<&str>(AId::Fill) != Some("yellow")
        || second.attribute::<&str>(AId::Fill) != Some("blue")
        || first.attribute::<&str>(AId::Stroke) != Some("green")
        || second.attribute::<&str>(AId::Stroke).is_some()
    {
        return Err("CSS source order, inline style, or pseudo-class contract changed".to_owned());
    }
    Ok(())
}

#[test]
fn explicit_inherit_resolves_ancestor_direct_parent_and_attribute_defaults() -> Result<(), String> {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg">
      <g opacity="0.25" fill="red"><g><rect id="descendant" fill="inherit" opacity="inherit" stroke="inherit"/></g>
      <rect id="direct" opacity="inherit"/></g></svg>"#;
    let document = parse_document(source, None)?;
    for (id, aid, expected) in [
        ("descendant", AId::Fill, "red"),
        ("descendant", AId::Opacity, "1"),
        ("descendant", AId::Stroke, "none"),
        ("direct", AId::Opacity, "0.25"),
    ] {
        assert_eq!(raw_attribute(&document, id, aid)?, expected);
    }
    Ok(())
}

#[test]
fn recursive_pattern_child_fill_is_neutralized() -> Result<(), String> {
    assert_attribute_value(
        include_str!(
            "../../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/paint-servers/pattern/self-recursive-on-child.svg"
        ),
        "rect1",
        AId::Fill,
        "none",
    )?;
    Ok(())
}

#[test]
fn mutually_recursive_pattern_strokes_are_broken_at_one_edge() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg">
      <pattern id="first-pattern"><rect id="first-child" stroke="url(#second-pattern)"/></pattern>
      <pattern id="second-pattern"><rect id="second-child" stroke="url(#first-pattern)"/></pattern>
    </svg>"##;
    let document = parse_document(source, None)?;
    assert_one_neutralized_link(&document, ["first-child", "second-child"], AId::Stroke)?;
    Ok(())
}

#[test]
fn recursive_clip_and_mask_child_links_are_neutralized() -> Result<(), String> {
    assert_recursive_pair(
        include_str!(
            "../../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/masking/clipPath/recursive-on-child.svg"
        ),
        ["path1", "path2"],
        AId::ClipPath,
    )?;
    assert_recursive_pair(
        include_str!(
            "../../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/masking/mask/recursive-on-child.svg"
        ),
        ["rect1", "rect2"],
        AId::Mask,
    )?;
    Ok(())
}

#[test]
fn recursive_feimage_filter_link_is_neutralized() -> Result<(), String> {
    assert_attribute_value(
        include_str!(
            "../../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/filters/feImage/recursive-links-1.svg"
        ),
        "rect3",
        AId::Filter,
        "none",
    )?;
    Ok(())
}

fn nested_svg(group_count: usize) -> String {
    let mut source = String::from("<svg xmlns=\"http://www.w3.org/2000/svg\">");
    for _ in 0..group_count {
        source.push_str("<g>");
    }
    source.push_str("<rect/>");
    for _ in 0..group_count {
        source.push_str("</g>");
    }
    source.push_str("</svg>");
    source
}

#[test]
fn parser_depth_limit_accepts_1024_and_rejects_1025() -> Result<(), String> {
    let accepted = nested_svg(1023);
    let accepted_xml = roxmltree::Document::parse(&accepted).map_err(|error| error.to_string())?;
    Document::parse_tree(&accepted_xml, None)
        .map_err(|error| format!("depth 1024 rejected: {error}"))?;

    let rejected = nested_svg(1024);
    let rejected_xml = roxmltree::Document::parse(&rejected).map_err(|error| error.to_string())?;
    if !matches!(
        Document::parse_tree(&rejected_xml, None),
        Err(roxmltree::Error::NodesLimitReached)
    ) {
        return Err("depth 1025 did not return NodesLimitReached".to_owned());
    }
    Ok(())
}
