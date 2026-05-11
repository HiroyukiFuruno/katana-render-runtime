use super::{
    MermaidJsRuntimeOps, MermaidRenderRequest, RuntimeBundleCache, ensure_svg, lock_cache,
    read_mermaid_bundle, rendered_svg,
};
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::runtime_assets::RuntimeAsset;
use std::collections::HashMap;
use std::sync::Mutex;

#[test]
fn fake_bundle_renders_svg_and_rejects_non_svg_output() {
    let path = std::env::temp_dir().join(format!(
        "kcf-mermaid-runtime-unit-{}.js",
        std::process::id()
    ));
    assert!(std::fs::write(&path, fake_bundle()).is_ok());
    assert!(read_mermaid_bundle(&path).is_ok());
    assert!(read_mermaid_bundle(&path).is_ok());

    let rendered =
        MermaidJsRuntimeOps::render("graph TD; A-->B", &path, DiagramColorPreset::dark());
    assert!(rendered.as_ref().is_ok_and(|svg| svg.contains("<svg")));
    assert!(ensure_svg("plain text").is_err());
    assert!(read_mermaid_bundle(std::path::Path::new("target/kcf-tests/missing.js")).is_err());
}

#[test]
fn zenuml_source_uses_v8_renderer_from_mermaid_surface() {
    let mermaid = RuntimeAsset::mermaid();
    let rendered = mermaid
        .materialize_at(mermaid.materialized_path())
        .and_then(|mermaid_js| {
            MermaidJsRuntimeOps::render(
                "zenuml\ntitle Surface Route\nA.method()",
                &mermaid_js,
                DiagramColorPreset::dark(),
            )
        });

    assert_zenuml_v8_svg(&rendered);
}

fn assert_zenuml_v8_svg(rendered: &Result<String, String>) {
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("<svg") && svg.contains("viewBox=")),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains("<foreignObject")),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains("data:image/png;base64,")),
        "{rendered:?}"
    );
}

#[test]
fn render_reports_missing_bundle_through_surface_path() {
    let result = MermaidJsRuntimeOps::render(
        "graph TD; A-->B",
        std::path::Path::new("target/kcf-tests/missing-mermaid-render.js"),
        DiagramColorPreset::dark(),
    );

    assert!(result.is_err());
}

#[test]
fn rendered_svg_rejects_plain_text_from_runtime() {
    assert!(rendered_svg("plain text".to_string()).is_err());
}

#[test]
fn poisoned_cache_reports_lock_error() {
    let cache: RuntimeBundleCache = Mutex::new(HashMap::new());
    let poison = std::panic::catch_unwind(|| poison_cache(&cache));

    assert!(poison.is_err());
    assert!(lock_cache(&cache).is_err());
    poison_cache(&cache);
}

fn poison_cache(cache: &RuntimeBundleCache) {
    let _guard = match cache.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    std::panic::resume_unwind(Box::new("poison mermaid cache"));
}

#[test]
fn request_id_changes_with_theme_and_source() {
    let dark = MermaidRenderRequest::new("graph TD; A-->B", DiagramColorPreset::dark());
    let light = MermaidRenderRequest::new("graph TD; A-->B", DiagramColorPreset::light());
    let other = MermaidRenderRequest::new("graph TD; B-->C", DiagramColorPreset::dark());

    assert!(dark.svg_id.starts_with("katana-mermaid-svg-"));
    assert_ne!(dark.svg_id, light.svg_id);
    assert_ne!(dark.svg_id, other.svg_id);
}

#[test]
fn request_fields_come_from_preset_not_global_state() {
    DiagramColorPreset::set_dark_mode(true);
    let request = MermaidRenderRequest::new("graph TD; A-->B", DiagramColorPreset::light());

    assert_eq!(request.theme, "default");
    assert_eq!(request.background, "transparent");
    assert_eq!(request.fill, "#fff2cc");
    assert_eq!(request.text, "#333333");
    assert_eq!(request.stroke, "#d6b656");
    assert_eq!(request.arrow, "#555555");
}

fn fake_bundle() -> &'static str {
    r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id, source) => ({
    svg: `<svg xmlns="http://www.w3.org/2000/svg" id="${id}" width="20" height="10" viewBox="0 0 20 10"><text>${source}</text></svg>`
  })
};
"#
}
