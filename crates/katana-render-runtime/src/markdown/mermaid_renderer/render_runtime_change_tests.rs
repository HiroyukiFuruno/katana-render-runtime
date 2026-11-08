use super::*;
use crate::markdown::runtime_resource_loader::RuntimeResourceLoader;

#[test]
fn runtime_content_partitions_svg_cache_at_same_path()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let dir = tempfile::tempdir()?;
    let runtime = dir.path().join("runtime.js");
    let source = format!("graph TD; U{}-->V", std::process::id());
    std::fs::write(&runtime, fake_mermaid_bundle_with_text("first-runtime"))?;
    let first = render_with_current_preset(&source, &runtime);
    std::fs::write(&runtime, fake_mermaid_bundle_with_text("second-runtime"))?;
    let second = render_with_current_preset(&source, &runtime);
    assert!(matches!(first, DiagramResult::Ok(svg) if svg.contains("first-runtime")));
    assert!(matches!(second, DiagramResult::Ok(svg) if svg.contains("second-runtime")));
    Ok(())
}

#[test]
fn unreadable_runtime_resources_report_the_original_source() -> TestResult<()> {
    let directory = tempfile::tempdir()?;
    let invalid_utf8 = directory.path().join("invalid.js");
    std::fs::write(&invalid_utf8, [0xff, 0xfe])?;
    let source = "graph TD; resource-->error";
    for runtime in [directory.path(), invalid_utf8.as_path()] {
        let result = render_with_current_preset(source, runtime);
        assert!(
            matches!(result, DiagramResult::Err { source: actual, error }
            if actual == source && error.contains("Failed to read Mermaid.js bundle"))
        );
    }
    Ok(())
}

#[test]
fn product_bundle_render_reports_cache_read_failure() -> TestResult<()> {
    let directory = tempfile::tempdir()?;
    let source = block("graph TD; cache-->read_error");
    let result = MermaidRenderOps::render_svg_with_bundle(
        &source,
        fake_mermaid_bundle(),
        DiagramColorPreset::current(),
        directory.path(),
    );
    assert!(
        matches!(result, DiagramResult::Err { source: actual, error }
        if actual == source.source && error.contains("Mermaid cache read failed"))
    );
    Ok(())
}

#[test]
fn product_bundle_render_rejects_invalid_runtime_without_publishing_cache() -> TestResult<()> {
    let directory = tempfile::tempdir()?;
    let cache = directory.path().join("missing.svg");
    let source = block("graph TD; runtime-->error");
    let result = MermaidRenderOps::render_svg_with_bundle(
        &source,
        "globalThis.mermaid = {};",
        DiagramColorPreset::current(),
        &cache,
    );
    assert!(
        matches!(result, DiagramResult::Err { source: actual, error }
        if actual == source.source && !error.is_empty())
    );
    assert!(!cache.exists());
    Ok(())
}
#[test]
fn blank_source_short_circuits_loaded_bundle_render() -> TestResult<()> {
    let directory = tempfile::tempdir()?;
    let resource = directory.path().join("mermaid.js");
    std::fs::write(&resource, "unused")?;
    let bundle = RuntimeResourceLoader::load(&resource, None)?;
    let result = MermaidRenderOps::render_mermaid_with_loaded(
        &block(" \n\t"),
        &resource,
        DiagramColorPreset::current(),
        &bundle,
    );
    assert!(matches!(result, DiagramResult::Ok(svg) if svg.is_empty()));
    Ok(())
}
