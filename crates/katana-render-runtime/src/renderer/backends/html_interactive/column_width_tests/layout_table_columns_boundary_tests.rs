use super::super::enforce_minimum_widths;
use super::{CssStyle, TableColumnWidths};
use crate::renderer::backends::html_document::HtmlDocument;
use crate::renderer::backends::html_interactive::document::table_rows;

const AVAILABLE_WIDTH: f32 = 0.00000001;
const DECLARED_WIDTH: &str = "0.00000002px";
const EXPECTED_WIDTH_EPSILON: f32 = 0.000000001;
const WIDTH_COMPARISON_EPSILON: f32 = 0.0001;

#[test]
fn subpixel_mixed_columns_keep_finite_available_budget() {
    let document = HtmlDocument::parse(&format!(
        "<table><tbody><tr><td style=\"width:{DECLARED_WIDTH};padding:0;border:0\"></td><td style=\"padding:0;border:0\"></td></tr></tbody></table>"
    ));
    let nodes = document.interactive_nodes_with_styles(&Default::default());
    let rows = table_rows(&nodes);
    let widths =
        TableColumnWidths::resolve(&rows, 2, AVAILABLE_WIDTH, &CssStyle::browser_default());

    assert_eq!(widths.len(), 2);
    assert!(
        widths
            .iter()
            .all(|width| width.is_finite() && *width >= 0.0)
    );
    assert!((widths.iter().sum::<f32>() - AVAILABLE_WIDTH).abs() <= EXPECTED_WIDTH_EPSILON);
    assert!((widths[0] - AVAILABLE_WIDTH).abs() <= EXPECTED_WIDTH_EPSILON);
    assert!(widths[1] <= EXPECTED_WIDTH_EPSILON);
}

#[test]
fn minimum_width_enforcement_shares_deficit_without_crossing_minimums() {
    let cases = [
        (
            "proportional reduction",
            [100.0, 80.0],
            [40.0, 20.0],
            120.0,
            [66.666664, 53.333332],
        ),
        (
            "repeated reduction after a clamp",
            [100.0, 50.0],
            [90.0, 0.0],
            100.0,
            [90.0, 10.0],
        ),
    ];

    for (name, preferred, minimums, available, expected) in cases {
        assert_minimum_width_case(name, preferred, minimums, available, expected);
    }
}

fn assert_minimum_width_case(
    name: &str,
    preferred: [f32; 2],
    minimums: [f32; 2],
    available: f32,
    expected: [f32; 2],
) {
    let actual = enforce_minimum_widths(preferred.to_vec(), minimums.to_vec(), available);
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| (*actual - expected).abs() < WIDTH_COMPARISON_EPSILON),
        "{name}: {actual:?}"
    );
    assert!(
        actual
            .iter()
            .zip(minimums)
            .all(|(actual, minimum)| *actual >= minimum)
    );
    assert!(actual.iter().sum::<f32>() <= available + WIDTH_COMPARISON_EPSILON);
}
