use super::test_support::fake_html_label_bundle;

pub(super) fn fake_bundle_with_cisco_placeholders() -> &'static str {
    static BUNDLE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BUNDLE
        .get_or_init(|| fake_html_label_bundle("20", "10", CISCO_PLACEHOLDER_BODY, ""))
        .as_str()
}

const CISCO_PLACEHOLDER_BODY: &str = r#"  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cisco");
  const fillPath = document.createElementNS("http://www.w3.org/2000/svg", "path");
  fillPath.setAttribute("fill", "fillcolor");
  fillPath.setAttribute("style", "fill: light-dark(fillcolor, #000000)");
  group.appendChild(fillPath);
  const secondaryFillStroke = document.createElementNS("http://www.w3.org/2000/svg", "path");
  secondaryFillStroke.setAttribute("fill", "none");
  secondaryFillStroke.setAttribute("stroke", "fillcolor2");
  secondaryFillStroke.setAttribute("style", "stroke: light-dark(fillcolor2, #000000)");
  group.appendChild(secondaryFillStroke);
  const secondaryStroke = document.createElementNS("http://www.w3.org/2000/svg", "path");
  secondaryStroke.setAttribute("fill", "none");
  secondaryStroke.setAttribute("stroke", "strokecolor2");
  group.appendChild(secondaryStroke);
  svg.appendChild(group);
"#;

pub(super) fn fake_bundle_with_unresolved_stencil_color() -> &'static str {
    static BUNDLE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BUNDLE
        .get_or_init(|| fake_html_label_bundle("20", "10", UNRESOLVED_STENCIL_COLOR_BODY, ""))
        .as_str()
}

const UNRESOLVED_STENCIL_COLOR_BODY: &str = r##"  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "salesforce");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("width", "20");
  rect.setAttribute("height", "10");
  rect.setAttribute("fill", "fillcolor2");
  rect.setAttribute("style", "fill: light-dark(fillcolor2, #000000)");
  group.appendChild(rect);
  svg.appendChild(group);
"##;
