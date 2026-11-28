use super::DrawioJsRuntimeOps;
use super::test_support::{fake_shadow_bundle, temp_runtime_path};
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_applies_named_style_shadow_and_padding() {
    let path = temp_runtime_path("kdr-drawio-shadow-unit");
    assert!(std::fs::write(&path, fake_shadow_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="cell" style="rounded=1;yellow" vertex="1"><mxGeometry x="0" y="0" width="20" height="10" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("drop-shadow"))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="31px""#))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="22px""#))
    );
}

#[test]
fn fake_bundle_applies_shadow_to_shape_group_without_transform() {
    let path = temp_runtime_path("kdr-drawio-untransformed-shadow-unit");
    let bundle = fake_shadow_bundle().replace(
        r#"  shape.setAttribute("transform", "translate(0.5,0.5)");"#,
        "",
    );
    assert!(std::fs::write(&path, bundle).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="cell" style="rounded=1;shadow=1" vertex="1"><mxGeometry x="0" y="0" width="20" height="10" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("drop-shadow")),
        "{rendered:?}"
    );
}
