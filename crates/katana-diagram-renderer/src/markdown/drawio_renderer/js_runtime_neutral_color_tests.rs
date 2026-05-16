use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_maps_adaptive_dark_black_fill_to_official_dark_white() {
    let path = temp_runtime_path("kdr-drawio-neutral-source-fill-unit");
    assert!(std::fs::write(&path, fake_neutral_source_fill_bundle()).is_ok());

    let source = r##"<mxGraphModel><root><mxCell id="cell" style="rounded=1;html=1;fillColor=#DDD;strokeColor=none;" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("fill=\"#ffffff\"")),
        "{rendered:?}"
    );
    assert!(!rendered.as_ref().is_ok_and(|svg| svg.contains("#ededed")));
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn fake_neutral_source_fill_bundle() -> &'static str {
    FAKE_NEUTRAL_SOURCE_FILL_BUNDLE
}

const FAKE_NEUTRAL_SOURCE_FILL_BUNDLE: &str = r##"
function Graph() {}
var mxUtils = {
  getLightDarkColor(value) {
    const key = String(value).toLowerCase();
    return {
      light: value,
      dark: key === "#ddd" ? "#000000" : "#2a353a",
      cssText: `light-dark(${value}, #000000)`,
    };
  },
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
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0");
  rect.setAttribute("y", "0");
  rect.setAttribute("width", "80");
  rect.setAttribute("height", "30");
  rect.setAttribute("fill", "#000000");
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
