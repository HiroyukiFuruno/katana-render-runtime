use super::{
    assert_arrow_centered_in_diagram, assert_arrow_centered_on_label, assert_block_arrow_width,
    block_arrow_dom_width,
};
use resvg::usvg::roxmltree::Document;
use rstest::rstest;

type TestResult = Result<(), Box<dyn std::error::Error>>;

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
