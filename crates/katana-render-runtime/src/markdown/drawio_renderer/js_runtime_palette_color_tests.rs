use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_RUNTIME_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[test]
fn fake_bundle_maps_infographic_light_palette_source_fill_to_official_dark() {
    assert_source_fill_color("#A680B8", "#8b6a9a", "#7f628c");
}

#[test]
fn fake_bundle_maps_infographic_mid_palette_source_fill_to_official_dark() {
    assert_source_fill_color("#719C59", "#5a7f45", "#719c59");
}

#[test]
fn fake_bundle_maps_infographic_heading_text_to_official_dark() {
    assert_source_text_color("#688F52", "fill=\"#668853\"", "fill=\"#688f52\"");
    assert_source_text_color("#856794", "fill=\"#9e84ab\"", "fill=\"#856794\"");
}

#[test]
fn fake_bundle_maps_dark_infographic_background_to_official_dark() {
    assert_source_fill_color("#282828", "#cbcbcb", "#282828");
    assert_source_fill_color("#08585C", "#79bec1", "#08585c");
}

#[test]
fn fake_bundle_maps_calendar_palette_to_official_dark() {
    assert_source_fill_color("#A6C9FF", "#284674", "#a6c9ff");
    assert_source_fill_color("#FFF787", "#2b2400", "#fff787");
}

#[test]
fn fake_bundle_maps_infographic_red_text_to_official_dark() {
    assert_source_text_color("#B85450", "fill=\"#d7817e\"", "fill=\"#b85450\"");
}

#[test]
fn fake_bundle_maps_evacuation_red_to_official_dark() {
    assert_source_fill_color("#FF0000", "#f79090", "#ff0000");
}

#[test]
fn fake_bundle_maps_named_white_path_fill_to_dark_canvas() {
    assert_source_path_fill_color("white", "fill=\"#121212\"", "fill=\"white\"");
}

fn assert_source_fill_color(source_color: &str, expected: &str, rejected: &str) {
    let path = temp_runtime_path("kdr-drawio-palette-fill-unit");
    assert!(std::fs::write(&path, fake_palette_fill_bundle()).is_ok());

    let source = format!(
        r##"<mxGraphModel><root><mxCell id="cell" style="rounded=1;html=1;fillColor={source_color};strokeColor=none;" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"##,
    );
    let rendered = DrawioJsRuntimeOps::render(&source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(&format!("fill=\"{expected}\""))),
        "{rendered:?}"
    );
    assert!(
        !rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(&format!("fill=\"{rejected}\""))),
        "{rendered:?}"
    );
}

fn assert_source_path_fill_color(source_color: &str, expected: &str, rejected: &str) {
    let path = temp_runtime_path("kdr-drawio-palette-path-fill-unit");
    assert!(std::fs::write(&path, fake_palette_path_fill_bundle()).is_ok());

    let source = format!(
        r##"<mxGraphModel><root><mxCell id="cell" style="shape=mxgraph.pid.vessels;html=1;fillColor={source_color};strokeColor=#ffffff;" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"##,
    );
    let rendered = DrawioJsRuntimeOps::render(&source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| svg.contains(expected)),
        "{rendered:?}"
    );
    assert!(
        !rendered.as_ref().is_ok_and(|svg| svg.contains(rejected)),
        "{rendered:?}"
    );
}

fn assert_source_text_color(source_color: &str, expected: &str, rejected: &str) {
    let path = temp_runtime_path("kdr-drawio-palette-text-unit");
    assert!(std::fs::write(&path, fake_palette_text_bundle()).is_ok());

    let source = format!(
        r##"<mxGraphModel><root><mxCell id="cell" value="&lt;font color=&quot;{source_color}&quot;&gt;Label&lt;/font&gt;" style="rounded=1;html=1;fontColor={source_color};" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"##,
    );
    let rendered = DrawioJsRuntimeOps::render(&source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| svg.contains(expected)),
        "{rendered:?}"
    );
    assert!(
        !rendered.as_ref().is_ok_and(|svg| svg.contains(rejected)),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    let counter = TEMP_RUNTIME_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}-{}-{counter}.js", std::process::id()))
}

fn fake_palette_fill_bundle() -> &'static str {
    FAKE_PALETTE_FILL_BUNDLE
}

const FAKE_PALETTE_FILL_BUNDLE: &str = r##"
function Graph() {}
var mxUtils = {
  getLightDarkColor(value) {
    return {
      light: value,
      dark: value,
      cssText: `light-dark(${value}, ${value})`,
    };
  },
};
var Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
function katanaSourceFillColor() {
  const source = String(globalThis.__katanaDrawioRequest?.source ?? "");
  return source.match(/fillColor=([^;"]+)/)?.[1] ?? "#A680B8";
}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "80");
  svg.setAttribute("height", "30");
  svg.setAttribute("viewBox", "0 0 80 30");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cell");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0");
  rect.setAttribute("y", "0");
  rect.setAttribute("width", "80");
  rect.setAttribute("height", "30");
  rect.setAttribute("fill", katanaSourceFillColor());
  group.appendChild(rect);
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

fn fake_palette_text_bundle() -> &'static str {
    FAKE_PALETTE_TEXT_BUNDLE
}

fn fake_palette_path_fill_bundle() -> &'static str {
    FAKE_PALETTE_PATH_FILL_BUNDLE
}

const FAKE_PALETTE_PATH_FILL_BUNDLE: &str = r##"
function Graph() {}
var mxUtils = {
  getLightDarkColor(value) {
    return {
      light: value,
      dark: value,
      cssText: `light-dark(${value}, ${value})`,
    };
  },
};
var Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
function katanaSourceFillColor() {
  const source = String(globalThis.__katanaDrawioRequest?.source ?? "");
  return source.match(/fillColor=([^;"]+)/)?.[1] ?? "white";
}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "80");
  svg.setAttribute("height", "30");
  svg.setAttribute("viewBox", "0 0 80 30");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cell");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", "M 0 0 L 80 0 L 80 30 L 0 30 Z");
  path.setAttribute("fill", katanaSourceFillColor());
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
"##;

const FAKE_PALETTE_TEXT_BUNDLE: &str = r##"
function Graph() {}
var mxUtils = {
  getLightDarkColor(value) {
    return {
      light: value,
      dark: value,
      cssText: `light-dark(${value}, ${value})`,
    };
  },
};
var Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
function katanaSourceFontColor() {
  const source = String(globalThis.__katanaDrawioRequest?.source ?? "");
  return source.match(/fontColor=([^;"]+)/)?.[1] ?? "#B85450";
}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "80");
  svg.setAttribute("height", "30");
  svg.setAttribute("viewBox", "0 0 80 30");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cell");
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.setAttribute("x", "40");
  text.setAttribute("y", "18");
  text.setAttribute("fill", katanaSourceFontColor());
  text.textContent = "Label";
  group.appendChild(text);
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
