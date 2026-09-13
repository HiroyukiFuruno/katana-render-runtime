use super::DrawioJsRuntimeOps;
use super::page_crop_test_support::{
    FAKE_BUNDLE_WITH_LEFT_TEXT_OVERFLOW, FAKE_BUNDLE_WITH_NEGATIVE_DISABLED_PAGE_BOUNDS,
    FAKE_BUNDLE_WITH_POSITIVE_TOP_PADDING, FAKE_BUNDLE_WITH_RENDERED_OVERFLOW,
    FAKE_BUNDLE_WITH_WIDE_WHITE_RECTANGLES, temp_runtime_path,
};
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_preserves_unmatched_disabled_page_svg_bounds() {
    let path = temp_runtime_path("kdr-drawio-disabled-page-bounds-unit");
    assert!(std::fs::write(&path, FAKE_BUNDLE_WITH_NEGATIVE_DISABLED_PAGE_BOUNDS).is_ok());

    let source = r#"<mxGraphModel page="0"><root><mxCell id="shape" parent="1" vertex="1"><mxGeometry x="10" y="880" width="246" height="60" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="1571px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="512px""#))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains(r#"translate(-121,62)"#))
    );
}

#[test]
fn fake_bundle_preserves_enabled_page_svg_bounds() {
    let path = temp_runtime_path("kdr-drawio-enabled-page-bounds-unit");
    assert!(std::fs::write(&path, FAKE_BUNDLE_WITH_NEGATIVE_DISABLED_PAGE_BOUNDS).is_ok());

    let source = r#"<mxGraphModel page="1"><root><mxCell id="shape" parent="1" vertex="1"><mxGeometry x="10" y="880" width="246" height="60" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="1571px""#))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="512px""#))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains(r#"translate(-121,62)"#))
    );
}

#[test]
fn fake_bundle_crops_from_the_rendered_model_in_a_multi_page_file() {
    let path = temp_runtime_path("kdr-drawio-active-page-crop-unit");
    assert!(std::fs::write(&path, FAKE_BUNDLE_WITH_POSITIVE_TOP_PADDING).is_ok());

    let source = r#"<mxfile>
<diagram name="Page-1"><mxGraphModel page="0"><root>
<mxCell id="shape" parent="1" vertex="1"><mxGeometry width="100" height="100" as="geometry"/></mxCell>
</root></mxGraphModel></diagram>
<diagram name="Page-2"><mxGraphModel page="0"><root>
<mxCell id="other" parent="1" vertex="1"><mxGeometry x="5000" y="5000" width="1000" height="1000" as="geometry"/></mxCell>
</root></mxGraphModel></diagram>
</mxfile>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="101px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="112px""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_preserves_linked_background_page_bounds() {
    let path = temp_runtime_path("kdr-drawio-linked-background-page-unit");
    assert!(std::fs::write(&path, FAKE_BUNDLE_WITH_POSITIVE_TOP_PADDING).is_ok());

    let source = r#"<mxfile>
<diagram id="foreground"><mxGraphModel page="0" backgroundImage="data:page/id,background"><root>
<mxCell id="shape" parent="1" vertex="1"><mxGeometry width="100" height="100" as="geometry"/></mxCell>
</root></mxGraphModel></diagram>
<diagram id="background"><mxGraphModel page="0"><root>
<mxCell id="background-shape" parent="1" vertex="1"><mxGeometry width="1000" height="1000" as="geometry"/></mxCell>
</root></mxGraphModel></diagram>
</mxfile>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="300px""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_prefers_negative_source_crop_box() {
    let path = temp_runtime_path("kdr-drawio-negative-source-crop-unit");
    assert!(std::fs::write(&path, FAKE_BUNDLE_WITH_RENDERED_OVERFLOW).is_ok());

    let source = r#"<mxGraphModel page="0"><root><mxCell id="1" parent="0"/><mxCell id="phone" parent="1" vertex="1"><mxGeometry x="-560" y="-490" width="390" height="780" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="391px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="781px""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_preserves_positive_top_source_crop_padding() {
    let path = temp_runtime_path("kdr-drawio-source-top-padding-unit");
    assert!(std::fs::write(&path, FAKE_BUNDLE_WITH_POSITIVE_TOP_PADDING).is_ok());

    let source = r#"<mxGraphModel page="0"><root><mxCell id="1" parent="0"/><mxCell id="shape" parent="1" vertex="1"><mxGeometry x="0" y="72" width="100" height="100" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="112px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains(r#"translate(0,-10)"#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_crops_disabled_page_to_rendered_label_overflow() {
    let path = temp_runtime_path("kdr-drawio-rendered-label-overflow-unit");
    assert!(std::fs::write(&path, FAKE_BUNDLE_WITH_LEFT_TEXT_OVERFLOW).is_ok());

    let source = r#"<mxGraphModel page="0"><root>
<mxCell id="shape" parent="1" vertex="1"><mxGeometry width="100" height="50" as="geometry"/></mxCell>
<mxCell id="label" value="Label" style="text;align=left;" parent="1" vertex="1"><mxGeometry width="100" height="20" as="geometry"/></mxCell>
</root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="121px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"transform="translate(20,"#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_preserves_source_authored_wide_bar_and_removes_label_background() {
    let path = temp_runtime_path("krr-drawio-wide-source-bar-unit");
    assert!(std::fs::write(&path, FAKE_BUNDLE_WITH_WIDE_WHITE_RECTANGLES).is_ok());

    let source = r##"<mxGraphModel page="0"><root>
<mxCell id="bar" value="" style="html=1;strokeColor=none;fillColor=#FFFFFF;" parent="1" vertex="1"><mxGeometry x="5" y="10" width="1400" height="10" as="geometry"/></mxCell>
<mxCell id="label" value="Title" style="html=1;strokeColor=none;fillColor=#FFFFFF;" parent="1" vertex="1"><mxGeometry x="5" y="30" width="1400" height="10" as="geometry"/></mxCell>
</root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"data-cell-id="bar""#)
                && svg.contains(r#"x="5" y="10" width="1400" height="10""#)
        }),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.matches(r#"width="1400""#).count() == 1),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_unions_aws_source_and_rendered_crop_bounds() {
    let path = temp_runtime_path("krr-drawio-aws-source-rendered-union-unit");
    assert!(std::fs::write(&path, FAKE_BUNDLE_WITH_POSITIVE_TOP_PADDING).is_ok());

    let source = r#"<mxGraphModel page="0"><root>
<mxCell id="1" parent="0"/>
<mxCell id="shape" style="shape=mxgraph.aws4.resourceIcon;" parent="1" vertex="1"><mxGeometry x="0" y="10" width="100" height="100" as="geometry"/></mxCell>
<mxCell id="source-only" style="shape=mxgraph.aws4.resourceIcon;" parent="1" vertex="1"><mxGeometry x="110" y="10" width="10" height="100" as="geometry"/></mxCell>
</root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"width="121px""#) && svg.contains(r#"height="112px""#)
        }),
        "{rendered:?}"
    );
}

#[path = "js_runtime_page_crop_geometry.rs"]
mod page_crop_geometry;

#[path = "js_runtime_page_crop_geometry_tests.rs"]
mod page_crop_geometry_tests;

#[path = "js_runtime_page_crop_source_fixtures.rs"]
mod page_crop_source_fixtures;

#[path = "js_runtime_page_crop_bundle_fixtures.rs"]
mod page_crop_bundle_fixtures;

#[path = "js_runtime_page_crop_sketch_tests.rs"]
mod page_crop_sketch_tests;

#[path = "js_runtime_page_crop_multipage_tests.rs"]
mod page_crop_multipage_tests;
