use super::DrawioJsRuntimeOps;
use super::test_support::fake_pixel_svg_bundle;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_pads_positive_infographic_export_top() {
    assert_export_top(
        "kdr-drawio-infographic-export-top-unit",
        r#"<mxGraphModel page="0"><root><mxCell id="1" parent="0"/><mxCell id="shape" style="shape=mxgraph.infographic.cylinder;" parent="1" vertex="1"><mxGeometry x="0" y="82" width="100" height="100" as="geometry"/></mxCell></root></mxGraphModel>"#,
        r#"viewBox="0 -10 100 110""#,
        r#"height="110px""#,
    );
}

#[test]
fn fake_bundle_pads_dark_cube_export_top() {
    assert_export_top(
        "kdr-drawio-dark-cube-export-top-unit",
        r##"<mxGraphModel page="1" background="#1A1A1A"><root><mxCell id="1" parent="0"/><mxCell id="shape" style="shape=cube;fillColor=#A680B8;" parent="1" vertex="1"><mxGeometry width="100" height="100" as="geometry"/></mxCell></root></mxGraphModel>"##,
        r#"viewBox="0 -10 100 110""#,
        r#"height="110px""#,
    );
}

#[test]
fn fake_bundle_does_not_pad_regular_page_export_top() {
    assert_export_top(
        "kdr-drawio-regular-export-top-unit",
        r#"<mxGraphModel page="1"><root><mxCell id="1" parent="0"/><mxCell id="shape" style="rounded=1;" parent="1" vertex="1"><mxGeometry width="100" height="100" as="geometry"/></mxCell></root></mxGraphModel>"#,
        r#"viewBox="0 0 100 100""#,
        r#"height="100px""#,
    );
}

fn assert_export_top(prefix: &str, source: &str, viewbox_fragment: &str, height_fragment: &str) {
    let path = temp_runtime_path(prefix);
    assert!(std::fs::write(&path, fake_bundle_with_top_aligned_content()).is_ok());

    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(viewbox_fragment)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(height_fragment)),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn fake_bundle_with_top_aligned_content() -> &'static str {
    static BUNDLE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BUNDLE
        .get_or_init(|| fake_pixel_svg_bundle("100", "100", TOP_ALIGNED_CONTENT_BODY, ""))
        .as_str()
}

const TOP_ALIGNED_CONTENT_BODY: &str = r#"  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "shape");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0");
  rect.setAttribute("y", "0");
  rect.setAttribute("width", "100");
  rect.setAttribute("height", "100");
  group.appendChild(rect);
  svg.appendChild(group);
"#;
