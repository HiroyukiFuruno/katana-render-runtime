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
        "kdr-mermaid-runtime-unit-{}.js",
        std::process::id()
    ));
    assert!(std::fs::write(&path, fake_bundle()).is_ok());
    assert!(read_mermaid_bundle(&path).is_ok());
    assert!(read_mermaid_bundle(&path).is_ok());

    let rendered =
        MermaidJsRuntimeOps::render("graph TD; A-->B", &path, DiagramColorPreset::dark());
    assert!(
        rendered.as_ref().is_ok_and(|svg| svg.contains("<svg")),
        "{rendered:?}"
    );
    assert!(ensure_svg("plain text").is_err());
    assert!(read_mermaid_bundle(std::path::Path::new("target/kdr-tests/missing.js")).is_err());
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

#[test]
fn runtime_keeps_ascii_kanban_overflow_at_reference_height() {
    let mermaid = RuntimeAsset::mermaid();
    let rendered = mermaid
        .materialize_at(mermaid.materialized_path())
        .and_then(|mermaid_js| {
            MermaidJsRuntimeOps::render(
                "kanban\n    Todo\n      [export runtime]\n    Doing\n      [Rust-managed Mermaid]\n    Done\n      [Remove OS Chrome path]",
                &mermaid_js,
                DiagramColorPreset::dark(),
            )
        });

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("viewBox=\"90 -310 630 94\"")),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains("height=\"56.6\"")),
        "{rendered:?}"
    );
}

#[test]
fn runtime_normalizes_treemap_value_metrics() {
    let mermaid = RuntimeAsset::mermaid();
    let rendered = mermaid
        .materialize_at(mermaid.materialized_path())
        .and_then(|mermaid_js| {
            MermaidJsRuntimeOps::render(
                "treemap\n    title Runtime cost\n    \"Mermaid\" : 45\n    \"DOM shim\" : 25\n    \"Rasterize\" : 20\n    \"Cache\" : 10",
                &mermaid_js,
                DiagramColorPreset::dark(),
            )
        });

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("viewBox=\"2 -2.34375 996 430.34375\"")),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("font-size: 23px") && svg.contains("fill: lightgrey")),
        "{rendered:?}"
    );
}

#[test]
fn runtime_normalizes_block_nbsp_arrow_width() {
    let mermaid = RuntimeAsset::mermaid();
    let rendered = mermaid
        .materialize_at(mermaid.materialized_path())
        .and_then(|mermaid_js| {
            MermaidJsRuntimeOps::render(
                "block-beta\ncolumns 1\n  db((\"DB\"))\n  blockArrowId6<[\"&nbsp;&nbsp;&nbsp;\"]>(down)\n  block:ID\n    A\n    B[\"A wide one in the middle\"]\n    C\n  end\n  space\n  D\n  ID --> D\n  C --> D\n  style B fill:#969,stroke:#333,stroke-width:4px",
                &mermaid_js,
                DiagramColorPreset::dark(),
            )
        });

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("viewBox=\"-5 -128.5 605.816")),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("id=\"katana-mermaid-svg")
                && svg.contains("-blockArrowId6\"")
                && svg.contains("points=\"28.73,0 0,-4 17.5,-4 17.5,-31")),
        "{rendered:?}"
    );
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
        std::path::Path::new("target/kdr-tests/missing-mermaid-render.js"),
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
