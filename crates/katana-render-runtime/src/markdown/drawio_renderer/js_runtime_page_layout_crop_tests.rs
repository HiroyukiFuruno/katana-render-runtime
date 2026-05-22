use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_crops_device_page_to_source_content_bounds() {
    let path = temp_runtime_path("kdr-drawio-page-layout-crop-unit");
    assert!(std::fs::write(&path, fake_bundle_with_wide_page_bounds()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render(device_page_source(), &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="1151px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="911px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"transform="translate(0,0)""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_ignores_wrapped_html_fallback_text_for_device_page_crop() {
    let path = temp_runtime_path("kdr-drawio-device-page-wrapped-text-crop-unit");
    assert!(std::fs::write(&path, fake_bundle_with_wrapped_html_fallback_text()).is_ok());

    let rendered = DrawioJsRuntimeOps::render(
        device_page_source_with_wrapped_html_text(),
        &path,
        DiagramColorPreset::dark(),
    );

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="1051px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains(r#"width="7235px""#)),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn device_page_source() -> &'static str {
    r#"<mxfile host="localhost" type="device"><diagram name="Page-1">
<mxGraphModel page="1" pageScale="1.5" pageWidth="826" pageHeight="1169" background="none"><root>
<mxCell id="1" parent="0"/>
<mxCell id="header" style="shadow=1" parent="1" vertex="1">
  <mxGeometry x="40" y="70" width="1150" height="40" as="geometry"/>
</mxCell>
<mxCell id="bottom" parent="1" vertex="1">
  <mxGeometry x="40" y="930" width="330" height="50" as="geometry"/>
</mxCell>
</root></mxGraphModel>
</diagram></mxfile>"#
}

fn device_page_source_with_wrapped_html_text() -> &'static str {
    r#"<mxfile host="localhost" type="device"><diagram name="Page-1">
<mxGraphModel page="1" pageScale="1" pageWidth="1100" pageHeight="850" background="none"><root>
<mxCell id="1" parent="0"/>
<mxCell id="container" parent="1" vertex="1">
  <mxGeometry x="30" y="20" width="1050" height="820" as="geometry"/>
</mxCell>
<mxCell id="label" value="A long wrapped label" style="text;html=1;whiteSpace=wrap;" parent="container" vertex="1">
  <mxGeometry x="20" y="370" width="570" height="240" as="geometry"/>
</mxCell>
</root></mxGraphModel>
</diagram></mxfile>"#
}

fn fake_bundle_with_wide_page_bounds() -> &'static str {
    FAKE_BUNDLE_WITH_WIDE_PAGE_BOUNDS
}

fn fake_bundle_with_wrapped_html_fallback_text() -> &'static str {
    FAKE_BUNDLE_WITH_WRAPPED_HTML_FALLBACK_TEXT
}

const FAKE_BUNDLE_WITH_WIDE_PAGE_BOUNDS: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "1848px");
  svg.setAttribute("height", "911px");
  svg.setAttribute("viewBox", "0 0 1848 911");
  svg.appendChild(createRectGroup("header", 0, 0, 1150, 40));
  svg.appendChild(createRectGroup("bottom", 0, 860, 330, 50));
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
function createRectGroup(id, x, y, width, height) {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", id);
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", String(x));
  rect.setAttribute("y", String(y));
  rect.setAttribute("width", String(width));
  rect.setAttribute("height", String(height));
  group.appendChild(rect);
  return group;
}
"#;

const FAKE_BUNDLE_WITH_WRAPPED_HTML_FALLBACK_TEXT: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "2000px");
  svg.setAttribute("height", "850px");
  svg.setAttribute("viewBox", "0 0 2000 850");
  svg.appendChild(createRectGroup("container", 0, 0, 1050, 820));
  svg.appendChild(createWrappedTextGroup("label"));
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
function createWrappedTextGroup(id) {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", id);
  const foreignObject = document.createElementNS("http://www.w3.org/2000/svg", "foreignObject");
  foreignObject.setAttribute("x", "20");
  foreignObject.setAttribute("y", "370");
  foreignObject.setAttribute("width", "570");
  foreignObject.setAttribute("height", "240");
  group.appendChild(foreignObject);
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.setAttribute("x", "20");
  text.setAttribute("y", "384");
  text.textContent = "This label is intentionally long and unwrapped in the SVG fallback ".repeat(80);
  group.appendChild(text);
  return group;
}
function createRectGroup(id, x, y, width, height) {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", id);
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", String(x));
  rect.setAttribute("y", String(y));
  rect.setAttribute("width", String(width));
  rect.setAttribute("height", String(height));
  group.appendChild(rect);
  return group;
}
"#;
