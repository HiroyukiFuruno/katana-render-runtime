use super::DrawioJsRuntimeOps;
use super::page_crop_test_support::create_rect_group_js;
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
            .is_ok_and(|svg| svg.contains(r#"width="1152px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="912px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains(r#"transform="translate(0,0)""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_includes_only_a_boundary_shadow_in_device_page_crop() {
    let path = temp_runtime_path("kdr-drawio-page-layout-shadow-crop-unit");
    assert!(std::fs::write(&path, fake_bundle_with_wide_page_bounds()).is_ok());

    let source = device_page_source_with_boundary_shadow();
    let rendered = DrawioJsRuntimeOps::render(&source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="1162px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="912px""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_ignores_implicit_wrapped_fallback_text_for_device_page_crop() {
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
            .is_ok_and(|svg| svg.contains(r#"width="1052px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains(r#"width="7235px""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_restores_relative_anchor_label_without_rewriting_parent_text() {
    let path = temp_runtime_path("kdr-drawio-relative-anchor-label-unit");
    assert!(std::fs::write(&path, FAKE_BUNDLE_WITH_EMPTY_RELATIVE_ANCHOR).is_ok());

    let rendered = DrawioJsRuntimeOps::render(
        device_page_source_with_relative_anchor(),
        &path,
        DiagramColorPreset::dark(),
    );

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(">List group item heading</tspan>")
                && svg.matches(">Parent body</tspan>").count() == 1
        }),
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
<mxCell id="header" parent="1" vertex="1">
  <mxGeometry x="40" y="70" width="1150" height="40" as="geometry"/>
</mxCell>
<mxCell id="bottom" parent="1" vertex="1">
  <mxGeometry x="40" y="930" width="330" height="50" as="geometry"/>
</mxCell>
</root></mxGraphModel>
</diagram></mxfile>"#
}

fn device_page_source_with_boundary_shadow() -> String {
    device_page_source().replace(
        r#"<mxCell id="header" parent="1" vertex="1">"#,
        r#"<mxCell id="header" style="shadow=1" parent="1" vertex="1">"#,
    )
}

fn device_page_source_with_relative_anchor() -> &'static str {
    r#"<mxfile type="device"><diagram><mxGraphModel page="1" background="none"><root>
<mxCell id="1" parent="0"/>
<mxCell id="parent" value="Parent body" style="html=1;whiteSpace=wrap;" vertex="1" parent="1">
  <mxGeometry x="40" y="480" width="400" height="80" as="geometry"/>
</mxCell>
<mxCell id="heading" value="List group item heading" style="html=1;shape=mxgraph.bootstrap.anchor;fontSize=18;" vertex="1" parent="parent">
  <mxGeometry width="400" height="40" relative="1" as="geometry"/>
</mxCell>
</root></mxGraphModel></diagram></mxfile>"#
}

fn device_page_source_with_wrapped_html_text() -> &'static str {
    r#"<mxfile host="localhost" type="device"><diagram name="Page-1">
<mxGraphModel page="1" pageScale="1" pageWidth="1100" pageHeight="850" background="none"><root>
<mxCell id="1" parent="0"/>
<mxCell id="container" parent="1" vertex="1">
  <mxGeometry x="30" y="20" width="1050" height="820" as="geometry"/>
</mxCell>
<mxCell id="label" value="A long wrapped label" style="text;whiteSpace=wrap;" parent="container" vertex="1">
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

const FAKE_BUNDLE_WITH_WIDE_PAGE_BOUNDS: &str = concat!(
    r#"
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
"#,
    create_rect_group_js!(),
    r#""#
);

const FAKE_BUNDLE_WITH_WRAPPED_HTML_FALLBACK_TEXT: &str = concat!(
    r#"
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
"#,
    create_rect_group_js!(),
    r#""#
);

const FAKE_BUNDLE_WITH_EMPTY_RELATIVE_ANCHOR: &str = concat!(
    r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "400px");
  svg.setAttribute("height", "80px");
  svg.setAttribute("viewBox", "0 0 400 80");
  const parent = createRectGroup("parent", 0, 0, 400, 80);
  const parentText = document.createElementNS("http://www.w3.org/2000/svg", "text");
  parentText.setAttribute("x", "12");
  parentText.setAttribute("y", "60");
  parentText.textContent = "Parent body";
  parent.appendChild(parentText);
  const heading = document.createElementNS("http://www.w3.org/2000/svg", "g");
  heading.setAttribute("data-cell-id", "heading");
  parent.appendChild(heading);
  svg.appendChild(parent);
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"#,
    create_rect_group_js!(),
    r#""#
);
