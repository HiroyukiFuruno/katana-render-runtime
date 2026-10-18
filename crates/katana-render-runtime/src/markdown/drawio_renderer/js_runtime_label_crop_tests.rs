use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_pads_aws_left_label_overflow() {
    let path = temp_runtime_path("kdr-drawio-aws-left-label-padding-unit");
    assert!(std::fs::write(&path, fake_bundle_with_left_label_crop()).is_ok());

    let source = r#"<mxGraphModel page="1"><root><mxCell id="aws" value="CloudFront &lt;br style=&quot;font-size: 16px;&quot;&gt;CDN" style="shape=mxgraph.aws4.resourceIcon;labelPosition=left;align=right;html=1;fontSize=16;" parent="1" vertex="1"><mxGeometry x="101" y="0" width="69" height="69" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="152px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"transform="translate(80,0)""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("margin-left: -2px")),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_offsets_right_label_after_left_padding() {
    let path = temp_runtime_path("kdr-drawio-aws-right-label-padding-unit");
    assert!(std::fs::write(&path, fake_bundle_with_left_and_right_labels()).is_ok());

    let source = r#"<mxGraphModel page="1"><root><mxCell id="left" value="CloudFront &lt;br&gt;CDN" style="shape=mxgraph.aws4.resourceIcon;labelPosition=left;align=right;html=1;fontSize=16;" parent="1" vertex="1"><mxGeometry x="101" y="0" width="69" height="69" as="geometry"/></mxCell><mxCell id="right" value="RDS &lt;br&gt;Slave" style="shape=mxgraph.aws4.resourceIcon;labelPosition=right;align=left;html=1;fontSize=16;" parent="1" vertex="1"><mxGeometry x="100" y="0" width="69" height="69" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("margin-left: 171px")),
        "{rendered:?}"
    );
    assert!(
        !rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("margin-left: 91px")),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_pads_rendered_centered_text_overflow() {
    let path = temp_runtime_path("kdr-drawio-centered-label-padding-unit");
    let bundle = fake_bundle_with_centered_text("Overflow");
    assert!(std::fs::write(&path, bundle).is_ok());

    let source = r#"<mxGraphModel page="1"><root><mxCell id="aws" value="Overflow" style="text;html=0;fontSize=28;fontStyle=1;" parent="1" vertex="1"><mxGeometry x="0" y="0" width="69" height="69" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            !svg.contains(r#"width="70px""#) && svg.contains(r#"transform="translate("#)
        }),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_pads_device_page_left_text_overflow_after_label_install() {
    let path = temp_runtime_path("kdr-drawio-device-page-label-padding-unit");
    let bundle = fake_bundle_with_centered_text("Beer");
    assert!(std::fs::write(&path, bundle).is_ok());

    let source = r#"<mxfile type="device"><diagram><mxGraphModel page="1" background="none"><root><mxCell id="aws" value="Beer" style="text;html=0;fontSize=28;fontStyle=1;" parent="1" vertex="1"><mxGeometry x="0" y="0" width="69" height="69" as="geometry"/></mxCell></root></mxGraphModel></diagram></mxfile>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            !svg.contains(r#"width="70px""#) && svg.contains(r#"transform="translate("#)
        }),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn fake_bundle_with_centered_text(text: &str) -> String {
    let injection = format!(
        r#"  group.appendChild(rect);
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.setAttribute("x", "0");
  text.setAttribute("y", "24");
  text.setAttribute("font-size", "28px");
  text.setAttribute("font-weight", "bold");
  text.setAttribute("text-anchor", "middle");
  text.textContent = "{text}";
  group.appendChild(text);"#,
    );
    FAKE_BUNDLE_WITH_LEFT_LABEL_CROP.replace("  group.appendChild(rect);", &injection)
}

fn fake_bundle_with_left_label_crop() -> &'static str {
    FAKE_BUNDLE_WITH_LEFT_LABEL_CROP
}

const FAKE_BUNDLE_WITH_LEFT_LABEL_CROP: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "70px");
  svg.setAttribute("height", "69px");
  svg.setAttribute("viewBox", "0 0 70 69");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "aws");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0.25");
  rect.setAttribute("y", "0");
  rect.setAttribute("width", "69");
  rect.setAttribute("height", "69");
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
"#;

fn fake_bundle_with_left_and_right_labels() -> &'static str {
    FAKE_BUNDLE_WITH_LEFT_AND_RIGHT_LABELS
}

const FAKE_BUNDLE_WITH_LEFT_AND_RIGHT_LABELS: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "170px");
  svg.setAttribute("height", "69px");
  svg.setAttribute("viewBox", "0 0 170 69");
  svg.appendChild(createGroup("left", 0.25));
  svg.appendChild(createGroup("right", 100));
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
function createGroup(id, x) {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", id);
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", String(x));
  rect.setAttribute("y", "0");
  rect.setAttribute("width", "69");
  rect.setAttribute("height", "69");
  group.appendChild(rect);
  return group;
}
"#;
