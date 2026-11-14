use super::DrawioJsRuntimeOps;
use super::test_support::{fake_color_bundle, fake_color_shape_group};
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_does_not_double_map_light_dark_style_color() {
    let path = temp_runtime_path("kdr-drawio-light-dark-style-unit");
    assert!(std::fs::write(&path, fake_light_dark_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="cell" style="rounded=1;html=1;" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("stop-color=\"rgb(139, 106, 154)\"")),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("style=\"stop-color: rgb(139, 106, 154)\"")),
        "{rendered:?}"
    );
    assert!(!rendered.as_ref().is_ok_and(|svg| svg.contains("#56425f")));
    assert!(!rendered.as_ref().is_ok_and(|svg| svg.contains("#674f72")));
}

#[test]
fn fake_bundle_uses_drawio_adaptive_dark_color_for_source_fill() {
    let path = temp_runtime_path("kdr-drawio-adaptive-source-fill-unit");
    assert!(std::fs::write(&path, fake_source_fill_bundle()).is_ok());

    let source = r##"<mxGraphModel><root><mxCell id="cell" style="rounded=1;html=1;fillColor=#CEDBE1;strokeColor=none;" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("fill=\"#2a353a\"")),
        "{rendered:?}"
    );
    assert!(!rendered.as_ref().is_ok_and(|svg| svg.contains("#1d1f20")));
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn fake_light_dark_bundle() -> String {
    let shape = fake_color_shape_group("url(#gradient)");
    let svg_body = [FAKE_LIGHT_DARK_SVG_DEFS, shape.as_str()].concat();
    fake_color_bundle(FAKE_LIGHT_DARK_MX_UTILS, &svg_body)
}

const FAKE_LIGHT_DARK_MX_UTILS: &str = r###"var mxUtils = {
  lightDarkColorSupported: false,
  preferDarkColor: true,
};
"###;

const FAKE_LIGHT_DARK_SVG_DEFS: &str = r###"  const defs = document.createElementNS("http://www.w3.org/2000/svg", "defs");
  const gradient = document.createElementNS("http://www.w3.org/2000/svg", "linearGradient");
  gradient.setAttribute("id", "gradient");
  const stop = document.createElementNS("http://www.w3.org/2000/svg", "stop");
  stop.setAttribute("offset", "0%");
  stop.setAttribute("stop-color", "#A680B8");
  if (mxUtils.lightDarkColorSupported) {
    stop.style.stopColor = "light-dark(rgb(166, 128, 184), rgb(139, 106, 154))";
  }
  gradient.appendChild(stop);
  defs.appendChild(gradient);
  svg.appendChild(defs);
"###;

fn fake_source_fill_bundle() -> String {
    fake_color_bundle(
        FAKE_SOURCE_FILL_MX_UTILS,
        &fake_color_shape_group("#CEDBE1"),
    )
}

const FAKE_SOURCE_FILL_MX_UTILS: &str = r###"var mxUtils = {
  getLightDarkColor(value) {
    return {
      light: value,
      dark: "#2a353a",
      cssText: `light-dark(${value}, #2a353a)`,
    };
  },
};
"###;
