use super::MermaidRenderOps;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::{DiagramBlock, DiagramKind, DiagramResult};

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[test]
fn render_mermaid_uses_explicit_runtime_path() -> TestResult<()> {
    let runtime = fake_mermaid_runtime("resolved");
    std::fs::write(&runtime, fake_mermaid_bundle())?;

    assert!(matches!(
        MermaidRenderOps::render_mermaid_with_runtime_path(
            &block("graph TD; R-->S"),
            &runtime,
            DiagramColorPreset::current()
        ),
        DiagramResult::Ok(svg) if svg.contains("<svg")
    ));
    Ok(())
}

#[test]
fn render_with_runtime_path_handles_cache_and_runtime_errors() -> TestResult<()> {
    let runtime = fake_mermaid_runtime("cache");
    std::fs::write(&runtime, fake_mermaid_bundle())?;
    let source = format!("graph TD; A{}-->B", std::process::id());

    let first = render_with_current_preset(&source, &runtime);
    assert!(matches!(first, DiagramResult::Ok(svg) if svg.contains("<svg")));

    std::fs::write(&runtime, "globalThis.mermaid = {};")?;
    let second = render_with_current_preset(&source, &runtime);
    assert!(matches!(second, DiagramResult::Ok(svg) if svg.contains("<svg")));

    let invalid_runtime = fake_mermaid_runtime("invalid");
    std::fs::write(&invalid_runtime, "globalThis.mermaid = {};")?;
    let error_source = format!("graph TD; E{}-->F", std::process::id());
    let failed = render_with_current_preset(&error_source, &invalid_runtime);
    assert!(matches!(failed, DiagramResult::Err { error, .. } if !error.is_empty()));
    Ok(())
}

#[test]
fn runtime_path_partitions_svg_cache() -> TestResult<()> {
    let first_runtime = fake_mermaid_runtime("cache-runtime-one");
    let second_runtime = fake_mermaid_runtime("cache-runtime-two");
    std::fs::write(
        &first_runtime,
        fake_mermaid_bundle_with_text("first-runtime"),
    )?;
    std::fs::write(
        &second_runtime,
        fake_mermaid_bundle_with_text("second-runtime"),
    )?;
    let source = format!("graph TD; P{}-->Q", std::process::id());

    let first = render_with_current_preset(&source, &first_runtime);
    let second = render_with_current_preset(&source, &second_runtime);

    assert!(matches!(first, DiagramResult::Ok(svg) if svg.contains("first-runtime")));
    assert!(matches!(second, DiagramResult::Ok(svg) if svg.contains("second-runtime")));
    Ok(())
}

#[test]
fn render_reports_missing_and_empty_inputs_without_runtime() {
    let missing = std::path::Path::new("target/kdr-tests/missing-mermaid.min.js");
    assert!(matches!(
        MermaidRenderOps::render_mermaid_with_runtime_path(
            &block("graph TD; A-->B"),
            missing,
            DiagramColorPreset::current()
        ),
        DiagramResult::NotInstalled { .. }
    ));
    assert!(matches!(
        MermaidRenderOps::render_mermaid_with_runtime_path(
            &block(" \n\t"),
            missing,
            DiagramColorPreset::current()
        ),
        DiagramResult::Ok(svg) if svg.is_empty()
    ));
}

#[test]
fn private_error_paths_are_explicit() {
    let source = block("graph TD; Z-->Q");
    assert_eq!(MermaidRenderOps::cache_profile(), "rust-managed-js-svg");
    assert!(MermaidRenderOps::ensure_cache_parent(std::path::Path::new("")).is_err());
    assert!(matches!(
        MermaidRenderOps::render_mermaid_with_cache_file(
            &source,
            std::path::Path::new("unused"),
            DiagramColorPreset::current(),
            std::path::Path::new("")
        ),
        DiagramResult::Err { .. }
    ));
    assert!(
        MermaidRenderOps::read_cached_svg(std::path::Path::new("target/kdr-tests/no-cache.svg"))
            .as_ref()
            .is_ok_and(Option::is_none)
    );
    assert_eq!(
        MermaidRenderOps::unique_svg_instance("<svg></svg>".to_string()),
        "<svg></svg>"
    );
}

#[test]
fn cache_parent_creation_errors_are_not_hidden() -> TestResult<()> {
    let parent_file =
        std::env::temp_dir().join(format!("kdr-cache-parent-file-{}", std::process::id()));
    std::fs::write(&parent_file, "not a directory")?;
    let cache_file = parent_file.join("cache.svg");

    assert!(MermaidRenderOps::ensure_cache_parent(&cache_file).is_err());
    std::fs::remove_file(parent_file)?;
    Ok(())
}

#[test]
fn cache_read_and_write_errors_are_not_hidden() -> TestResult<()> {
    let cache_dir = std::env::temp_dir().join(format!("kdr-cache-dir-{}", std::process::id()));
    std::fs::create_dir_all(&cache_dir)?;
    let block = block("graph TD; X-->Y");

    assert!(MermaidRenderOps::read_cached_svg(&cache_dir).is_err());
    assert!(matches!(
        MermaidRenderOps::render_svg(
            &block,
            std::path::Path::new("unused"),
            DiagramColorPreset::current(),
            &cache_dir
        ),
        DiagramResult::Err { .. }
    ));
    assert!(matches!(
        MermaidRenderOps::write_cached_svg(&block, &cache_dir, "<svg></svg>".to_string()),
        DiagramResult::Err { .. }
    ));
    Ok(())
}

fn block(source: &str) -> DiagramBlock {
    DiagramBlock {
        kind: DiagramKind::Mermaid,
        source: source.to_string(),
    }
}

fn render_with_current_preset(source: &str, runtime: &std::path::Path) -> DiagramResult {
    MermaidRenderOps::render_mermaid_with_runtime_path(
        &block(source),
        runtime,
        DiagramColorPreset::current(),
    )
}

fn fake_mermaid_runtime(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("kdr-mermaid-{name}-{}.js", std::process::id()))
}

fn fake_mermaid_bundle() -> &'static str {
    fake_mermaid_bundle_with_text("${source}")
}

fn fake_mermaid_bundle_with_text(text: &'static str) -> &'static str {
    match text {
        "${source}" => FAKE_MERMAID_BUNDLE_WITH_SOURCE,
        "first-runtime" => FAKE_MERMAID_BUNDLE_WITH_FIRST_RUNTIME,
        "second-runtime" => FAKE_MERMAID_BUNDLE_WITH_SECOND_RUNTIME,
        _ => unreachable!("unsupported fake Mermaid bundle text"),
    }
}

const FAKE_MERMAID_BUNDLE_WITH_SOURCE: &str = r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id, source) => ({
    svg: `<svg xmlns="http://www.w3.org/2000/svg" id="${id}" width="20" height="10" viewBox="0 0 20 10"><text>${source}</text></svg>`
  })
};
"#;

const FAKE_MERMAID_BUNDLE_WITH_FIRST_RUNTIME: &str = r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => ({
    svg: `<svg xmlns="http://www.w3.org/2000/svg" id="${id}" width="20" height="10" viewBox="0 0 20 10"><text>first-runtime</text></svg>`
  })
};
"#;

const FAKE_MERMAID_BUNDLE_WITH_SECOND_RUNTIME: &str = r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => ({
    svg: `<svg xmlns="http://www.w3.org/2000/svg" id="${id}" width="20" height="10" viewBox="0 0 20 10"><text>second-runtime</text></svg>`
  })
};
"#;
