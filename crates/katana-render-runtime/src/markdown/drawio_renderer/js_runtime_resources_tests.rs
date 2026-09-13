use super::archive::{
    DrawioResourceArchiveGroup, compressed_group_contents, group_bytes_for_test, resource_contents,
    selected_groups, validate_resource_archive,
};
use super::selector::{
    DrawioResourceSelector, drawio_prefix, extract_resource_groups, resource_groups,
};
use super::{DrawioResourceCatalog, encoding_for_path, mime_type_for_path};
use crate::markdown::runtime_asset_archive::RuntimeAssetArchive;
use std::time::Instant;

#[test]
fn builtin_selects_basic_stencil_shape_scripts_and_referenced_images() -> Result<(), String> {
    let source = r#"
        <mxCell style="shape=mxgraph.ios7ui.button;image=img/lib/azure2/general/File.svg"/>
    "#;
    let resources = DrawioResourceCatalog::builtin(source)?;

    assert!(resources.iter().any(|it| it.path == "stencils/basic.xml"));
    assert!(
        resources
            .iter()
            .any(|it| it.path == "stencils/ios7/misc.xml")
    );
    assert!(resources.iter().any(|it| it.path.starts_with("shapes/")));
    assert!(
        resources
            .iter()
            .any(|it| it.path == "img/lib/azure2/general/File.svg")
    );
    Ok(())
}

#[test]
fn resource_archive_validation_reports_length_and_bounds_errors() -> Result<(), String> {
    assert!(validate_resource_archive(vec![1], 2).is_err());
    assert!(validate_resource_archive(vec![1], 1).is_ok());
    assert!(resource_contents(&[1], "overflow", usize::MAX, 2).is_err());
    assert!(resource_contents(&[1], "out-of-bounds", 0, 2).is_err());
    assert!(matches!(resource_contents(&[1], "valid", 0, 1), Ok([1])));
    assert!(compressed_group_contents(&[1], usize::MAX, 2).is_err());
    assert!(compressed_group_contents(&[1], 0, 2).is_err());
    let empty_group = compressed_group_contents(&[], 0, 0)?;
    assert!(RuntimeAssetArchive::brotli(empty_group).is_err());
    Ok(())
}

#[test]
fn invalid_resource_group_stops_before_brotli_decompression() {
    let invalid_group = DrawioResourceArchiveGroup {
        compressed_start: usize::MAX,
        compressed_length: 1,
        uncompressed_length: 0,
        index: &[],
    };

    assert!(group_bytes_for_test(&invalid_group).is_err());
}

#[test]
fn basic_diagram_selects_only_its_small_resource_group() -> Result<(), String> {
    let source = "<mxGraphModel><root/></mxGraphModel>";
    let selector = DrawioResourceSelector::new(source);
    let selected = selected_groups(&selector);
    let selected_bytes: usize = selected.iter().map(|group| group.uncompressed_length).sum();
    let archive_bytes: usize = super::archive::DRAWIO_RESOURCE_ARCHIVE_GROUPS
        .iter()
        .map(|group| group.uncompressed_length)
        .sum();

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].index, [("stencils/basic.xml", 0, 43_925)]);
    assert_eq!(selected_bytes, 43_925);
    assert!(selected_bytes < archive_bytes);
    assert!(
        super::archive::DRAWIO_RESOURCE_ARCHIVE_GROUPS
            .iter()
            .filter(|group| !selected
                .iter()
                .any(|selected_group| std::ptr::eq(*selected_group, *group)))
            .any(|group| group.uncompressed_length > selected_bytes)
    );

    let resources = DrawioResourceCatalog::builtin(source)?;
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].path, "stencils/basic.xml");
    assert_eq!(resources[0].content.len(), selected_bytes);
    Ok(())
}

#[test]
fn repeated_basic_diagram_collection_remains_small_and_deterministic() -> Result<(), String> {
    let source = "<mxGraphModel><root/></mxGraphModel>";
    let started = Instant::now();
    for _ in 0..8 {
        let resources = DrawioResourceCatalog::builtin(source)?;
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].path, "stencils/basic.xml");
    }
    eprintln!(
        "eight basic Draw.io resource collections completed in {:?}",
        started.elapsed()
    );
    Ok(())
}

#[test]
fn mime_type_for_path_covers_supported_assets() {
    assert_eq!(mime_type_for_path("a.xml"), "text/xml");
    assert_eq!(mime_type_for_path("a.js"), "application/javascript");
    assert_eq!(mime_type_for_path("a.svg"), "image/svg+xml");
    assert_eq!(mime_type_for_path("a.png"), "image/png");
    assert_eq!(mime_type_for_path("a.jpg"), "image/jpeg");
    assert_eq!(mime_type_for_path("a.jpeg"), "image/jpeg");
    assert_eq!(mime_type_for_path("a.gif"), "image/gif");
    assert_eq!(mime_type_for_path("a.bin"), "application/octet-stream");
}

#[test]
fn encoding_for_path_covers_binary_and_text_assets() {
    assert!(matches!(
        encoding_for_path("a.png"),
        super::DrawioResourceEncoding::Base64
    ));
    assert_eq!(encoding_for_path("a.xml").as_str(), "text");
    assert_eq!(encoding_for_path("a.png").as_str(), "base64");
}

#[test]
fn resource_groups_and_prefix_handle_known_values() {
    assert_eq!(resource_groups("rackGeneral"), vec!["rack".to_string()]);
    assert_eq!(resource_groups("custom"), vec!["custom".to_string()]);
    assert!(extract_resource_groups("shape=mxgraph.rackGeneral.server").contains("rack"));
    assert_eq!(drawio_prefix(";"), None);
}
