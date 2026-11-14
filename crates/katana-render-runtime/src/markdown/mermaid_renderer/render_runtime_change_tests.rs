use super::*;
use crate::markdown::runtime_assets::RuntimeAsset;
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

#[test]
fn real_runtime_cache_hit_survives_runtime_removal_and_keeps_cached_bytes() -> TestResult<()> {
    let directory = tempfile::tempdir()?;
    let runtime = materialized_runtime(directory.path())?;
    let cache = directory.path().join("cache.svg");
    let source = block("graph TD; A-->B");
    assert!(!cache.exists());
    let first = render_with_owned_cache(&source, &runtime, &cache)?;
    let bytes = std::fs::read(&cache)?;
    std::fs::rename(&runtime, directory.path().join("held-mermaid.min.js"))?;
    assert!(!runtime.exists());
    let cold_cache = directory.path().join("cold.svg");
    assert!(render_with_owned_cache(&source, &runtime, &cold_cache).is_err());
    assert!(!cold_cache.exists());
    let second = render_with_owned_cache(&source, &runtime, &cache)?;
    assert_eq!(std::fs::read(&cache)?, bytes);
    assert_ne!(svg_root_id(&first)?, svg_root_id(&second)?);
    Ok(())
}

#[test]
fn real_runtime_syntax_error_preserves_source_and_does_not_publish_cache() -> TestResult<()> {
    let directory = tempfile::tempdir()?;
    let runtime = materialized_runtime(directory.path())?;
    let cache = directory.path().join("invalid.svg");
    let source = block("graph TD\nA-->");
    assert!(!cache.exists());
    let result = MermaidRenderOps::render_mermaid_with_cache_file(
        &source,
        &runtime,
        DiagramColorPreset::current(),
        &cache,
    );
    assert!(
        matches!(result, DiagramResult::Err { source: actual, error }
        if actual == source.source && !error.is_empty())
    );
    assert!(!cache.exists());
    let control = directory.path().join("valid.svg");
    let svg = render_with_owned_cache(&block("graph TD; A-->B"), &runtime, &control)?;
    svg_root_id(&svg)?;
    assert!(control.is_file());
    Ok(())
}

fn materialized_runtime(directory: &std::path::Path) -> TestResult<std::path::PathBuf> {
    Ok(RuntimeAsset::mermaid().materialize_at(directory.join("mermaid.min.js"))?)
}

fn render_with_owned_cache(
    source: &DiagramBlock,
    runtime: &std::path::Path,
    cache: &std::path::Path,
) -> TestResult<String> {
    match MermaidRenderOps::render_mermaid_with_cache_file(
        source,
        runtime,
        DiagramColorPreset::current(),
        cache,
    ) {
        DiagramResult::Ok(svg) => Ok(svg),
        result => Err(format!("real Mermaid rendering failed: {result:?}").into()),
    }
}

fn svg_root_id(svg: &str) -> TestResult<String> {
    let document = resvg::usvg::roxmltree::Document::parse(svg)?;
    let root = document.root_element();
    if root.tag_name().name() != "svg" {
        return Err("rendered root is not SVG".into());
    }
    let id = root.attribute("id").ok_or("rendered SVG has no ID")?;
    if !id.starts_with("katana-mermaid-svg-") {
        return Err("rendered SVG has an unexpected ID".into());
    }
    Ok(id.to_owned())
}

const CACHE_PARENT_CHILD: &str = "KRR_TEST_MERMAID_CACHE_PARENT_CHILD";
const CACHE_PARENT_SENTINEL: &[u8] = b"owned cache parent blocker";

#[test]
fn bundled_render_reports_cache_parent_failure_in_isolated_process() -> TestResult<()> {
    if let Some(mode) = std::env::var_os(CACHE_PARENT_CHILD) {
        return check_cache_parent_child(&mode);
    }
    for blocked in [true, false] {
        let directory = tempfile::tempdir()?;
        let parent = directory.path().join("katana_mermaid_cache");
        if blocked {
            std::fs::write(&parent, CACHE_PARENT_SENTINEL)?;
        }
        run_cache_parent_child(directory.path(), blocked)?;
        if blocked {
            assert_eq!(std::fs::read(&parent)?, CACHE_PARENT_SENTINEL);
        }
    }
    Ok(())
}

fn run_cache_parent_child(directory: &std::path::Path, blocked: bool) -> TestResult<()> {
    let (_, module) = module_path!()
        .split_once("::")
        .ok_or("test module missing")?;
    let name = format!("{module}::bundled_render_reports_cache_parent_failure_in_isolated_process");
    let output = crate::system::ProcessService::create_command(std::env::current_exe()?)
        .args([
            name.as_str(),
            "--exact",
            "--test-threads=1",
            "--color=never",
        ])
        .env(
            CACHE_PARENT_CHILD,
            if blocked { "blocked" } else { "normal" },
        )
        .env("TMPDIR", directory)
        .env("TMP", directory)
        .env("TEMP", directory)
        .output()?;
    let stdout = std::str::from_utf8(&output.stdout)?;
    let passed = format!("test {name} ... ok");
    assert!(output.status.success(), "isolated Mermaid child failed");
    assert_eq!(stdout.lines().filter(|line| *line == passed).count(), 1);
    Ok(())
}

fn check_cache_parent_child(mode: &std::ffi::OsStr) -> TestResult<()> {
    let directory =
        std::path::PathBuf::from(std::env::var_os("TMPDIR").ok_or("child temp missing")?);
    assert!(
        std::env::temp_dir().canonicalize()? == directory.canonicalize()?,
        "isolated temp directory mismatch"
    );
    let runtime = directory.join("bundled-mermaid.js");
    let source = block("graph TD; A-->B");
    let parent = directory.join("katana_mermaid_cache");
    if mode == "blocked" {
        check_blocked_cache_parent(&source, &runtime, &parent)
    } else if mode == "normal" {
        check_normal_cache_parent(&source, &runtime, &parent)
    } else {
        Err("unknown isolated child mode".into())
    }
}

fn check_blocked_cache_parent(
    source: &DiagramBlock,
    runtime: &std::path::Path,
    parent: &std::path::Path,
) -> TestResult<()> {
    assert_eq!(std::fs::read(parent)?, CACHE_PARENT_SENTINEL);
    let result = MermaidRenderOps::render_mermaid_with_origin(
        source,
        runtime,
        DiagramColorPreset::current(),
        true,
    );
    assert!(
        matches!(result, DiagramResult::Err { source: actual, error }
        if actual == source.source && error.contains("Mermaid cache create parent failed"))
    );
    assert_eq!(std::fs::read(parent)?, CACHE_PARENT_SENTINEL);
    Ok(())
}

fn check_normal_cache_parent(
    source: &DiagramBlock,
    runtime: &std::path::Path,
    parent: &std::path::Path,
) -> TestResult<()> {
    assert!(!parent.exists());
    let result = MermaidRenderOps::render_mermaid_with_origin(
        source,
        runtime,
        DiagramColorPreset::current(),
        true,
    );
    let DiagramResult::Ok(svg) = result else {
        return Err("bundled control failed".into());
    };
    svg_root_id(&svg)?;
    assert!(parent.is_dir());
    Ok(())
}
