#[path = "js_runtime_coverage_gap_architecture_tests.rs"]
mod architecture;
#[path = "js_runtime_geometry_boundary_tests.rs"]
mod boundary;
#[path = "js_runtime_coverage_gap_c4_tests.rs"]
mod c4;
#[path = "js_runtime_coverage_gap_git_tests.rs"]
mod git;

#[test]
fn treemap_rejects_missing_leaf_identity() -> Result<(), String> {
    let svg = super::tests::treemap_control();
    super::GeometryContract::treemap(&svg)?;
    let changed = svg.replacen(">Mermaid</text>", ">Unknown</text>", 1);
    assert_ne!(changed, svg);
    assert_eq!(
        super::GeometryContract::treemap(&changed).err().as_deref(),
        Some("semantic element must be unique")
    );
    Ok(())
}
