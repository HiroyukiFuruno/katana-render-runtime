use super::{BLOCK, layout};
#[path = "js_runtime_block_style_boundary_test_support.rs"]
mod support;
use support::{label_rect, parse, with_parent_style, with_stylesheet};

#[test]
fn fixture_helpers_reject_invalid_xml_and_missing_geometry() -> Result<(), String> {
    assert!(parse("<svg>").is_err());
    let missing_node = parse("<svg/>")?;
    assert!(label_rect(&missing_node).is_err());
    let missing_rect = parse(r#"<svg><g class="node" id="id-ID"/></svg>"#)?;
    assert!(label_rect(&missing_rect).is_err());
    Ok(())
}

#[test]
fn stylesheet_font_rejects_missing_fields_and_invalid_size() -> Result<(), String> {
    for svg in [
        "<svg/>",
        "<svg><style>font-size:16px;</style></svg>",
        "<svg><style>font-family:Noto Sans;</style></svg>",
        "<svg><style>font-family:Noto Sans;font-size:invalidpx;</style></svg>",
    ] {
        let document = parse(svg)?;
        assert!(
            layout::stylesheet_font(document.root_element()).is_err(),
            "accepted: {svg}"
        );
    }
    let document = parse("<svg><style>font-family:Noto Sans;font-size:16px;</style></svg>")?;
    assert_eq!(
        layout::stylesheet_font(document.root_element())?,
        ("Noto Sans", 16.0)
    );
    Ok(())
}

#[test]
fn block_label_style_requires_each_declaration() -> Result<(), String> {
    for (style, missing) in [
        ("stroke:#333;stroke-width:4px", "fill:#969"),
        ("fill:#969;stroke-width:4px", "stroke:#333"),
        ("fill:#969;stroke:#333", "stroke-width:4px"),
    ] {
        let svg = with_parent_style(style);
        let document = parse(&svg)?;
        let error = layout::assert_label_style(label_rect(&document)?)
            .err()
            .ok_or("invalid style accepted")?;
        assert!(error.contains(missing), "unexpected error: {error}");
    }
    let svg = with_parent_style("fill:#969;stroke:#333;stroke-width:4px");
    let document = parse(&svg)?;
    layout::assert_label_style(label_rect(&document)?)
}

#[test]
fn svg_number_rejects_missing_and_non_numeric_width() -> Result<(), String> {
    for (replacement, expected) in [
        ("", "missing SVG attribute"),
        ("width=\"invalid\"", "invalid float literal"),
    ] {
        let svg = BLOCK.replacen("width=\"250\"", replacement, 1);
        let document = parse(&svg)?;
        let error = layout::svg_number(label_rect(&document)?, "width")
            .err()
            .ok_or("invalid width accepted")?;
        assert!(error.contains(expected), "unexpected error: {error}");
    }
    let document = parse(BLOCK)?;
    assert_eq!(layout::svg_number(label_rect(&document)?, "width")?, 250.0);
    Ok(())
}

#[test]
fn label_width_propagates_missing_stylesheet_and_rejects_wrong_width() -> Result<(), String> {
    let document = parse(BLOCK)?;
    assert_eq!(
        layout::assert_block_label_width(document.root_element(), label_rect(&document)?).err(),
        Some("missing diagram stylesheet".to_owned())
    );
    let svg = with_stylesheet("font-family:Noto Sans;font-size:16px;").replacen(
        "width=\"250\"",
        "width=\"0\"",
        1,
    );
    let document = parse(&svg)?;
    let error = layout::assert_block_label_width(document.root_element(), label_rect(&document)?)
        .err()
        .ok_or("wrong width accepted")?;
    assert!(
        error.contains("Block label width 0 differs"),
        "unexpected error: {error}"
    );
    Ok(())
}
