use super::*;

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
