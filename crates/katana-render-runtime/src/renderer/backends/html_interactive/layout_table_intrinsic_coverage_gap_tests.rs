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

#[test]
fn parsed_nowrap_inline_br_keeps_nested_line_widths_separate() -> Result<(), String> {
    let width = parsed_nowrap_width("<span>AA<br><small>BBBBB</small></span>")?;
    let no_break = parsed_nowrap_width("<span>AABBBBB</span>")?;
    assert!(width > 0.0 && width < no_break);
    Ok(())
}

#[test]
fn parsed_nowrap_intrinsic_preserves_break_and_atomic_boundaries() -> Result<(), String> {
    let one_break = parsed_nowrap_width("<span>AA<br>BBBBB</span>")?;
    let two_breaks = parsed_nowrap_width("<span>AA<br><br>BBBBB</span>")?;
    let adjacent = parsed_nowrap_width("<span>AA</span><small>BBBBB</small>")?;
    let normal = parsed_nowrap_width("<span style=\"white-space:normal\">AA BBBBB</span>")?;
    let atomic = parsed_nowrap_width("<span style=\"display:inline-flex\">AA<br>BBBBB</span>")?;
    let atomic_prefix =
        parsed_nowrap_width("<span style=\"display:inline-flex\">AA<br>BBBBB</span>X")?;
    assert_eq!(one_break, two_breaks);
    assert!(adjacent > one_break && normal < adjacent);
    assert!(atomic_prefix > atomic);
    Ok(())
}

fn parsed_nowrap_width(content: &str) -> Result<f32, String> {
    let markup = format!(
        "<table><tr><td id=target style=\"white-space:nowrap\">{content}</td></tr></table>"
    );
    let document = HtmlDocument::parse(&markup);
    let nodes = document.interactive_nodes_with_styles(&HashMap::new());
    projected_target_cell_width(&nodes)
}
