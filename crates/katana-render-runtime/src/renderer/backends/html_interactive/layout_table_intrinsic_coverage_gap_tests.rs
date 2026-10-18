use super::tests::projected_target_cell_width;
use crate::renderer::backends::html_document::HtmlDocument;
use std::collections::HashMap;

#[test]
fn parsed_target_cell_without_nowrap_reports_the_specific_contract_error() {
    let document = HtmlDocument::parse("<table><tr><td id=target>content</td></tr></table>");
    let nodes = document.interactive_nodes_with_styles(&HashMap::new());

    assert_eq!(
        projected_target_cell_width(&nodes),
        Err("target cell must retain nowrap style".to_string())
    );
}
