use super::super::{
    DiagramColorPreset, MermaidJsRuntimeOps, RuntimeAsset, block_nbsp_arrow_source,
};
use super::{
    assert_arrow_centered_in_diagram, assert_arrow_centered_on_label, assert_block_arrow_width,
    block_arrow_dom_width,
};
use resvg::usvg::roxmltree::Document;
use rstest::rstest;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn rendered_block_rejects_unmeasurable_font_sizes() -> TestResult {
    let asset = RuntimeAsset::mermaid();
    let path = asset.materialize_at(asset.materialized_path())?;
    let svg =
        MermaidJsRuntimeOps::render(block_nbsp_arrow_source(), &path, DiagramColorPreset::dark())?;
    let document = Document::parse(&svg)?;
    assert!(block_arrow_dom_width(document.root_element())? > 0.0);
    let range = stylesheet_font_size_range(&document)?;
    for invalid in ["-1", "1e100"] {
        let mut changed = svg.clone();
        changed.replace_range(range.clone(), invalid);
        let parsed = Document::parse(&changed)?;
        assert_eq!(
            block_arrow_dom_width(parsed.root_element())
                .err()
                .as_deref(),
            Some("Failed to rasterize SVG: SVG font metrics unavailable")
        );
    }
    Ok(())
}

fn stylesheet_font_size_range(
    document: &Document<'_>,
) -> Result<std::ops::Range<usize>, Box<dyn std::error::Error>> {
    let style = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.children().find(|child| child.is_text()))
        .ok_or("stylesheet text missing")?;
    let content = style.text().ok_or("stylesheet text missing")?;
    let start = content.find("font-size:").ok_or("font size missing")? + "font-size:".len();
    let end = start
        + content[start..]
            .find("px;")
            .ok_or("font size unit missing")?;
    let offset = style.range().start;
    Ok(offset + start..offset + end)
}

#[test]
fn width_oracle_rejects_a_polygon_that_differs_from_measured_text() -> TestResult {
    let document =
        Document::parse("<svg><style>font-family:Noto Sans;font-size:16px;</style></svg>")?;
    let root = document.root_element();
    let expected = block_arrow_dom_width(root)?;
    assert!(expected > 0.0);
    assert_eq!(
        assert_block_arrow_width(root, [0.0, -1.0, expected, 0.0])?,
        expected
    );
    let error = assert_block_arrow_width(root, [0.0, -1.0, expected + 1.0, 0.0])
        .err()
        .ok_or("incorrect polygon width was accepted")?;
    assert!(error.contains("differs from DOM text bounds"));
    Ok(())
}

#[rstest]
#[case::centered(50, true)]
#[case::displaced(49, false)]
fn diagram_center_oracle_checks_the_viewbox_center(
    #[case] translation: i32,
    #[case] valid: bool,
) -> TestResult {
    let svg =
        format!("<svg viewBox='0 0 100 40'><g transform='translate({translation} 0)'/></svg>");
    let document = Document::parse(&svg)?;
    let root = document.root_element();
    let arrow = root
        .children()
        .find(|node| node.is_element())
        .ok_or("arrow missing")?;
    let result = assert_arrow_centered_in_diagram(root, arrow);
    assert_eq!(result.is_ok(), valid);
    if let Err(error) = result {
        assert_eq!(error, "Block arrow is not centered on the diagram");
    }
    Ok(())
}

#[rstest]
#[case::missing_viewbox("<svg><g transform='translate(50 0)'/></svg>")]
#[case::invalid_viewbox("<svg viewBox='invalid'><g transform='translate(50 0)'/></svg>")]
#[case::missing_transform("<svg viewBox='0 0 100 40'><g/></svg>")]
#[case::invalid_transform("<svg viewBox='0 0 100 40'><g transform='invalid'/></svg>")]
fn diagram_center_oracle_rejects_invalid_geometry(#[case] svg: &str) -> TestResult {
    let document = Document::parse(svg)?;
    let root = document.root_element();
    let arrow = root
        .children()
        .find(|node| node.is_element())
        .ok_or("arrow missing")?;
    assert!(assert_arrow_centered_in_diagram(root, arrow).is_err());
    Ok(())
}

#[rstest]
#[case::centered(-0.5, 1.0, true)]
#[case::displaced(0.0, 1.0, false)]
#[case::wrong_width(-0.5, 2.0, false)]
fn label_center_oracle_checks_position_and_measured_width(
    #[case] translation: f64,
    #[case] width: f64,
    #[case] valid: bool,
) -> TestResult {
    let svg = format!("<polygon transform='translate({translation} 0)'/>");
    let document = Document::parse(&svg)?;
    let result =
        assert_arrow_centered_on_label(document.root_element(), [0.0, -1.0, 1.0, 0.0], width);
    assert_eq!(result.is_ok(), valid);
    if let Err(error) = result {
        assert_eq!(
            error,
            "Block arrow polygon is not centered on its measured label"
        );
    }
    Ok(())
}
