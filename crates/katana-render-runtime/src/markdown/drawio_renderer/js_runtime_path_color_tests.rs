use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_maps_opaque_black_path_to_official_dark_connector() {
    let path = temp_runtime_path("kdr-drawio-opaque-black-path-unit");
    assert!(std::fs::write(&path, fake_black_path_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="cell" edge="1"><mxGeometry relative="1" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(
            |svg| svg.contains(r##"<path d="M 0 0 L 80 0" fill="#ffffff" stroke="#ffffff""##,)
        ),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_maps_opaque_black_vertex_path_to_official_dark_white() {
    let path = temp_runtime_path("kdr-drawio-opaque-black-vertex-path-unit");
    assert!(std::fs::write(&path, fake_black_path_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="cell" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(
            |svg| svg.contains(r##"<path d="M 0 0 L 80 0" fill="#ffffff" stroke="#ffffff""##,)
        ),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_maps_swimlane_black_path_to_official_dark_white() {
    let path = temp_runtime_path("kdr-drawio-swimlane-black-path-unit");
    assert!(std::fs::write(&path, fake_black_path_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="cell" style="swimlane;html=1;" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(
            |svg| svg.contains(r##"<path d="M 0 0 L 80 0" fill="#ffffff" stroke="#ffffff""##,)
        ),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_maps_transparent_black_path_to_official_dark_white() {
    let path = temp_runtime_path("kdr-drawio-transparent-black-path-unit");
    assert!(std::fs::write(&path, fake_black_path_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="cell" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r##"fill="#ffffff" fill-opacity="0.1""##)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_maps_android_device_screen_path_to_transparent() {
    let path = temp_runtime_path("kdr-drawio-android-device-screen-unit");
    assert!(std::fs::write(&path, fake_android_device_bundle()).is_ok());

    let source = r##"<mxfile><diagram name="Page-1"><mxGraphModel><root><mxCell id="0"/><mxCell id="1" parent="0"/><mxCell id="cell" style="shape=mxgraph.android.phone2;fillColor=#ffffff;strokeColor=#c0c0c0;" parent="1" vertex="1"><mxGeometry x="0" y="0" width="200" height="390" as="geometry"/></mxCell></root></mxGraphModel></diagram></mxfile>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| svg
            .contains(r##"<path d="M 8 35 L 8 355 L 193 355 L 193 35 Z" fill="transparent""##,)),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn fake_black_path_bundle() -> &'static str {
    FAKE_BLACK_PATH_BUNDLE
}

const FAKE_BLACK_PATH_BUNDLE: &str = r##"
function Graph() {}
var mxUtils = {
  lightDarkColorSupported: false,
  preferDarkColor: true,
};
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "80");
  svg.setAttribute("height", "30");
  svg.setAttribute("viewBox", "0 0 80 30");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cell");
  const connector = document.createElementNS("http://www.w3.org/2000/svg", "path");
  connector.setAttribute("d", "M 0 0 L 80 0");
  connector.setAttribute("fill", "#000000");
  connector.setAttribute("stroke", "#000000");
  const overlay = document.createElementNS("http://www.w3.org/2000/svg", "path");
  overlay.setAttribute("d", "M 0 10 L 80 10 L 80 20 L 0 20 Z");
  overlay.setAttribute("fill", "#000000");
  overlay.setAttribute("fill-opacity", "0.1");
  group.appendChild(connector);
  group.appendChild(overlay);
  svg.appendChild(group);
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"##;

fn fake_android_device_bundle() -> &'static str {
    FAKE_ANDROID_DEVICE_BUNDLE
}

const FAKE_ANDROID_DEVICE_BUNDLE: &str = r##"
function Graph() {}
var mxUtils = {
  lightDarkColorSupported: false,
  preferDarkColor: true,
};
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "201");
  svg.setAttribute("height", "391");
  svg.setAttribute("viewBox", "0 0 201 391");
  const root = document.createElementNS("http://www.w3.org/2000/svg", "g");
  root.setAttribute("data-cell-id", "0");
  const layer = document.createElementNS("http://www.w3.org/2000/svg", "g");
  layer.setAttribute("data-cell-id", "1");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cell");
  const transform = document.createElementNS("http://www.w3.org/2000/svg", "g");
  transform.setAttribute("transform", "translate(0.5,0.5)");
  const body = document.createElementNS("http://www.w3.org/2000/svg", "path");
  body.setAttribute("d", "M 0 36 L 200 36 L 200 390 L 0 390 Z");
  body.setAttribute("fill", "#ffffff");
  const screen = document.createElementNS("http://www.w3.org/2000/svg", "path");
  screen.setAttribute("d", "M 8 35 L 8 355 L 193 355 L 193 35 Z");
  screen.setAttribute("fill", "#ededed");
  transform.appendChild(body);
  transform.appendChild(screen);
  group.appendChild(transform);
  layer.appendChild(group);
  root.appendChild(layer);
  svg.appendChild(root);
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"##;
