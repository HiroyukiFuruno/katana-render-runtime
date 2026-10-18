use super::{CssStyle, TableColumnWidths};
use crate::renderer::backends::html_document::HtmlDocument;
use crate::renderer::backends::html_interactive::document::table_rows;

const AVAILABLE_WIDTH: f32 = 0.00000001;
const DECLARED_WIDTH: &str = "0.00000002px";
const EXPECTED_WIDTH_EPSILON: f32 = 0.000000001;

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
