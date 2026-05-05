use super::{
    DrawioResourceCatalog, drawio_prefix, encoding_for_path, mime_type_for_path, resource_groups,
};

#[test]
fn builtin_selects_basic_stencil_shape_scripts_and_referenced_images() {
    let source = r#"
        <mxCell style="shape=mxgraph.ios7ui.button;image=img/lib/azure2/general/File.svg"/>
    "#;
    let resources = DrawioResourceCatalog::builtin(source);

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
    assert_eq!(drawio_prefix(";"), None);
}
