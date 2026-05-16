use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_aligns_positive_page_origin_to_source_vertex_bounds() {
    let path = temp_runtime_path("kdr-drawio-page-origin-unit");
    assert!(std::fs::write(&path, fake_bundle_with_shifted_page_origin()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render(page_origin_source(), &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="1478px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="315px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"transform="translate(-43,-160)""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_preserves_scaled_page_origin() {
    let path = temp_runtime_path("kdr-drawio-scaled-page-origin-unit");
    assert!(std::fs::write(&path, fake_bundle_with_shifted_page_origin()).is_ok());

    let rendered = DrawioJsRuntimeOps::render(
        scaled_page_origin_source(),
        &path,
        DiagramColorPreset::dark(),
    );

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="1507px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains(r#"transform="translate(-43,0)""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_preserves_non_image_page_origin() {
    let path = temp_runtime_path("kdr-drawio-non-image-page-origin-unit");
    assert!(std::fs::write(&path, fake_bundle_with_shifted_page_origin()).is_ok());

    let rendered = DrawioJsRuntimeOps::render(
        non_image_page_origin_source(),
        &path,
        DiagramColorPreset::dark(),
    );

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="1507px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains(r#"transform="translate(-43,0)""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_rounds_disabled_page_infographic_near_integer_paths() {
    let path = temp_runtime_path("kdr-drawio-infographic-rounding-unit");
    assert!(std::fs::write(&path, fake_bundle_with_near_integer_path()).is_ok());

    let rendered = DrawioJsRuntimeOps::render(
        disabled_page_infographic_source(),
        &path,
        DiagramColorPreset::dark(),
    );

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"d="M 10 20 L 513 118 L 190 168""#)),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn page_origin_source() -> &'static str {
    r#"<mxfile type="device"><diagram><mxGraphModel page="1" background="none"><root>
<mxCell id="1" parent="0"/>
<mxCell id="arrow" style="html=1;" vertex="1" parent="1">
  <mxGeometry x="160" y="198.44" width="120" height="62.17" as="geometry"/>
</mxCell>
<mxCell id="image" style="shape=image;html=1;image=data:image/svg+xml,PHN2Zy8+;" vertex="1" parent="1">
  <mxGeometry x="250" y="170" width="160.03" height="119.05" as="geometry"/>
</mxCell>
<mxCell id="label" value="Cells on membrane" style="html=1;fontSize=18;" vertex="1" parent="1">
  <mxGeometry x="1340" y="170.005" width="282.85" height="110" as="geometry"/>
</mxCell>
</root></mxGraphModel></diagram></mxfile>"#
}

fn scaled_page_origin_source() -> &'static str {
    r#"<mxGraphModel page="1" pageScale="1.5"><root>
<mxCell id="1" parent="0"/>
<mxCell id="arrow" style="html=1;" vertex="1" parent="1">
  <mxGeometry x="160" y="198.44" width="120" height="62.17" as="geometry"/>
</mxCell>
<mxCell id="image" style="shape=image;html=1;image=data:image/svg+xml,PHN2Zy8+;" vertex="1" parent="1">
  <mxGeometry x="250" y="170" width="160.03" height="119.05" as="geometry"/>
</mxCell>
<mxCell id="label" value="Cells on membrane" style="html=1;fontSize=18;" vertex="1" parent="1">
  <mxGeometry x="1340" y="170.005" width="282.85" height="110" as="geometry"/>
</mxCell>
</root></mxGraphModel>"#
}

fn non_image_page_origin_source() -> &'static str {
    r#"<mxGraphModel page="1"><root>
<mxCell id="1" parent="0"/>
<mxCell id="arrow" style="html=1;" vertex="1" parent="1">
  <mxGeometry x="160" y="198.44" width="120" height="62.17" as="geometry"/>
</mxCell>
<mxCell id="image" style="shape=rect;html=1;" vertex="1" parent="1">
  <mxGeometry x="250" y="170" width="160.03" height="119.05" as="geometry"/>
</mxCell>
<mxCell id="label" value="Cells on membrane" style="html=1;fontSize=18;" vertex="1" parent="1">
  <mxGeometry x="1340" y="170.005" width="282.85" height="110" as="geometry"/>
</mxCell>
</root></mxGraphModel>"#
}

fn disabled_page_infographic_source() -> &'static str {
    r#"<mxGraphModel page="0"><root>
<mxCell id="1" parent="0"/>
<mxCell id="shape" style="shape=mxgraph.infographic.cylinder;" vertex="1" parent="1">
  <mxGeometry x="0" y="0" width="120" height="60" as="geometry"/>
</mxCell>
</root></mxGraphModel>"#
}

fn fake_bundle_with_shifted_page_origin() -> &'static str {
    FAKE_BUNDLE_WITH_SHIFTED_PAGE_ORIGIN
}

const FAKE_BUNDLE_WITH_SHIFTED_PAGE_ORIGIN: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "1507px");
  svg.setAttribute("height", "299px");
  svg.setAttribute("viewBox", "0 0 1507 299");
  svg.appendChild(createRectGroup("arrow", 43, 188.44, 120, 62.17));
  svg.appendChild(createRectGroup("image", 133, 160, 160.03, 119.05));
  svg.appendChild(createLabelGroup());
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
function createLabelGroup() {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "label");
  group.setAttribute("font-size", "18px");
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.setAttribute("x", "1364.42");
  text.setAttribute("y", "294");
  text.textContent = "Cells on membrane";
  group.appendChild(text);
  return group;
}
"#;

fn fake_bundle_with_near_integer_path() -> &'static str {
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
  svg.setAttribute("width", "600px");
  svg.setAttribute("height", "200px");
  svg.setAttribute("viewBox", "0 0 600 200");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "shape");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", "M 10 20 L 512.99 117.99 L 190.01 167.95");
  group.appendChild(path);
  svg.appendChild(group);
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"#
}
