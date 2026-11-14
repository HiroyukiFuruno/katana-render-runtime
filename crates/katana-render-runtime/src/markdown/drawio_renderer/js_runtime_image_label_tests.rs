use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_positions_default_image_html_label_below_image() {
    let path = temp_runtime_path("kdr-drawio-default-image-label-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="image" value="Network Switch&lt;br&gt;" style="image;html=1;image=img/lib/clip_art/networking/Switch_128x128.png;fontSize=12;fontColor=#0A3DA3;" vertex="1" /></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert_rendered_contains(&rendered, "width: 1px");
    assert_rendered_contains(&rendered, "align-items: unsafe flex-start");
    assert_rendered_contains(&rendered, "padding-top: 89px");
    assert_rendered_contains(&rendered, "margin-left: 140px");
    assert_rendered_contains(&rendered, "white-space: nowrap");
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn assert_rendered_contains(rendered: &Result<String, String>, needle: &str) {
    assert!(
        rendered.as_ref().is_ok_and(|svg| svg.contains(needle)),
        "{rendered:?}"
    );
}

fn fake_bundle() -> &'static str {
    static BUNDLE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BUNDLE
        .get_or_init(|| {
            super::test_support::fake_html_label_bundle(
                "400",
                "220",
                FAKE_BUNDLE_BODY,
                FAKE_BUNDLE_AFTER_CALLBACK,
            )
        })
        .as_str()
}

const FAKE_BUNDLE_BODY: &str = r#"  svg.appendChild(createImageGroup("image", 100, 40, 80, 42));
"#;

const FAKE_BUNDLE_AFTER_CALLBACK: &str = r#"function createImageGroup(id, x, y, width, height) {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", id);
  const image = document.createElementNS("http://www.w3.org/2000/svg", "image");
  image.setAttribute("x", String(x));
  image.setAttribute("y", String(y));
  image.setAttribute("width", String(width));
  image.setAttribute("height", String(height));
  group.appendChild(image);
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.textContent = id;
  group.appendChild(text);
  return group;
}
"#;
