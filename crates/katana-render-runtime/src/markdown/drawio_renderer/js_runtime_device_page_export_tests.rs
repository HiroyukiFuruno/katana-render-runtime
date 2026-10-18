use super::DrawioJsRuntimeOps;
use super::device_page_test_support::{
    device_page_with_source_top_padding, fake_bundle_with_device_page_content, temp_runtime_path,
};
use crate::markdown::color_preset::DiagramColorPreset;

const SOURCE_TOP_PADDING_FOREIGN_OBJECT_HOOK: &str = r#"  group.appendChild(rect);
  const foreignObject = document.createElementNS("http://www.w3.org/2000/svg", "foreignObject");
  const outer = document.createElement("div");
  outer.setAttribute("style", "display: flex; padding-top: 31px; margin-left: 0px;");
  outer.textContent = "Label";
  foreignObject.appendChild(outer);
  group.appendChild(foreignObject);"#;

const SYMMETRIC_IMPLICIT_PAGE_MARGIN_SOURCE: &str = r#"<mxfile type="device"><diagram><mxGraphModel page="1"><root>
<mxCell id="1" parent="0"/>
<mxCell id="shape" style="shape=rect;strokeColor=none;" vertex="1" parent="1">
  <mxGeometry x="0" y="10" width="100" height="64" as="geometry"/>
</mxCell>
</root></mxGraphModel></diagram></mxfile>"#;

#[test]
fn fake_bundle_preserves_aws_device_page_when_rendered_content_fills_canvas() {
    let path = temp_runtime_path("krr-drawio-aws-full-page-canvas-unit");
    assert!(std::fs::write(&path, aws_full_page_bundle()).is_ok());

    let source = device_page_source_with_shape(
        "shape=rect;mxgraph.aws;mxgraph.aws;mxgraph.aws;mxgraph.aws;mxgraph.aws;mxgraph.aws;mxgraph.aws;mxgraph.aws;mxgraph.aws;mxgraph.aws;mxgraph.aws;mxgraph.aws;",
        "<mxGeometry width=\"100\" height=\"60\" as=\"geometry\"/>",
    );
    let rendered = DrawioJsRuntimeOps::render(&source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"viewBox="0 0 300 200""#)
                && svg.contains(r#"width="300px""#)
                && svg.contains(r#"height="200px""#)
                && !svg.contains(r#"transform="translate("#)
        }),
        "{rendered:?}"
    );
}

fn aws_full_page_bundle() -> String {
    fake_bundle_with_device_page_content()
        .replace(
            r#"rect.setAttribute("width", "100");"#,
            r#"rect.setAttribute("width", "298");"#,
        )
        .replace(
            r#"rect.setAttribute("height", "60");"#,
            r#"rect.setAttribute("height", "197");"#,
        )
}

#[test]
fn fake_bundle_does_not_pad_intentional_negative_source_origin() {
    let path = temp_runtime_path("krr-drawio-negative-source-page-origin-unit");
    assert!(std::fs::write(&path, negative_source_origin_bundle()).is_ok());

    let source = device_page_source_with_shape(
        "shape=rect;",
        "<mxGeometry y=\"-40\" width=\"300\" height=\"200\" as=\"geometry\"/>",
    );
    let rendered = DrawioJsRuntimeOps::render(&source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"viewBox="0 0 300 200""#)
                && svg.contains(r#"height="200px""#)
                && !svg.contains(r#"transform="translate(0,12)""#)
        }),
        "{rendered:?}"
    );
}

fn negative_source_origin_bundle() -> String {
    fake_bundle_with_device_page_content()
        .replace(
            r#"rect.setAttribute("y", "0");"#,
            r#"rect.setAttribute("y", "-12");"#,
        )
        .replace(
            r#"rect.setAttribute("width", "100");"#,
            r#"rect.setAttribute("width", "300");"#,
        )
        .replace(
            r#"rect.setAttribute("height", "60");"#,
            r#"rect.setAttribute("height", "212");"#,
        )
}

fn device_page_source_with_shape(style: &str, geometry: &str) -> String {
    format!(
        r#"<mxfile type="device"><diagram><mxGraphModel page="1"><root>
<mxCell id="1" parent="0"/>
<mxCell id="shape" style="{style}" vertex="1" parent="1">
  {geometry}
</mxCell>
</root></mxGraphModel></diagram></mxfile>"#
    )
}

#[test]
fn fake_bundle_preserves_source_top_padding_for_shapes_labels_and_canvas() {
    let path = temp_runtime_path("kdr-drawio-device-page-top-padding-unit");
    assert!(std::fs::write(&path, source_top_padding_bundle()).is_ok());

    let rendered = DrawioJsRuntimeOps::render(
        device_page_with_source_top_padding(),
        &path,
        DiagramColorPreset::dark(),
    );

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"height="62px""#)
                && svg.contains(r#"transform="translate(1,0)""#)
                && svg.contains(r#"<rect x="-0.55" y="1.45""#)
                && svg.contains("padding-top: 31px")
                && !svg.contains("translate(-0.45,0.55)")
        }),
        "{rendered:?}"
    );
}

fn source_top_padding_bundle() -> String {
    fake_bundle_with_device_page_content()
        .replace(
            r#"rect.setAttribute("x", "0");"#,
            r#"rect.setAttribute("x", "-0.55");"#,
        )
        .replace(
            r#"rect.setAttribute("y", "0");"#,
            r#"rect.setAttribute("y", "1.45");"#,
        )
        .replace(
            "  group.appendChild(rect);",
            SOURCE_TOP_PADDING_FOREIGN_OBJECT_HOOK,
        )
}

#[test]
fn fake_bundle_removes_symmetric_implicit_page_margin() {
    let path = temp_runtime_path("kdr-drawio-implicit-page-margin-unit");
    assert!(std::fs::write(&path, symmetric_implicit_page_margin_bundle()).is_ok());

    let rendered = DrawioJsRuntimeOps::render(
        SYMMETRIC_IMPLICIT_PAGE_MARGIN_SOURCE,
        &path,
        DiagramColorPreset::dark(),
    );

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"viewBox="0 0 100 64""#)
                && svg.contains(r#"transform="translate(0,-10)""#)
        }),
        "{rendered:?}"
    );
}

fn symmetric_implicit_page_margin_bundle() -> String {
    fake_bundle_with_device_page_content()
        .replace(
            r#"svg.setAttribute("width", "300px");"#,
            r#"svg.setAttribute("width", "100px");"#,
        )
        .replace(
            r#"svg.setAttribute("height", "200px");"#,
            r#"svg.setAttribute("height", "84px");"#,
        )
        .replace(
            r#"svg.setAttribute("viewBox", "0 0 300 200");"#,
            r#"svg.setAttribute("viewBox", "0 0 100 84");"#,
        )
        .replace(
            r#"rect.setAttribute("y", "0");"#,
            r#"rect.setAttribute("y", "10");"#,
        )
        .replace(
            r#"rect.setAttribute("height", "60");"#,
            r#"rect.setAttribute("height", "64");"#,
        )
}

#[test]
fn fake_bundle_aligns_scaled_full_page_paint_to_export_edge() {
    let path = temp_runtime_path("krr-drawio-scaled-full-page-edge-unit");
    assert!(std::fs::write(&path, scaled_full_page_bundle()).is_ok());

    let source = r##"<mxfile type="device"><diagram><mxGraphModel page="1" pageScale="1.5" background="#ffffff"><root>
<mxCell id="1" parent="0"/>
<mxCell id="shape" style="shape=rect;strokeColor=none;" vertex="1" parent="1">
  <mxGeometry y="2" width="300" height="198" as="geometry"/>
</mxCell>
</root></mxGraphModel></diagram></mxfile>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"viewBox="0 0 300 200""#)
                && svg.contains(r#"transform="translate(0,-1)""#)
        }),
        "{rendered:?}"
    );
}

fn scaled_full_page_bundle() -> String {
    fake_bundle_with_device_page_content()
        .replace(
            r#"rect.setAttribute("y", "0");"#,
            r#"rect.setAttribute("y", "2");"#,
        )
        .replace(
            r#"rect.setAttribute("width", "100");"#,
            r#"rect.setAttribute("width", "300");"#,
        )
        .replace(
            r#"rect.setAttribute("height", "60");"#,
            r#"rect.setAttribute("height", "198");"#,
        )
}

#[test]
fn fake_bundle_removes_dense_network_export_edge() {
    let path = temp_runtime_path("krr-drawio-dense-network-edge-unit");
    assert!(std::fs::write(&path, dense_network_bundle()).is_ok());

    let cells = (0..99)
        .map(|index| format!(r#"<mxCell id="dummy-{index}" parent="1"/>"#))
        .collect::<String>();
    let source = format!(
        r#"<mxfile><diagram><mxGraphModel page="1"><root>
<mxCell id="1" parent="0"/>
<mxCell id="shape" style="shape=mxgraph.networks.router;" vertex="1" parent="1">
  <mxGeometry width="300" height="200" as="geometry"/>
</mxCell>
{cells}
</root></mxGraphModel></diagram></mxfile>"#
    );
    let rendered = DrawioJsRuntimeOps::render(&source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"viewBox="0 0 300 199""#) && svg.contains(r#"height="199px""#)
        }),
        "{rendered:?}"
    );
}

fn dense_network_bundle() -> String {
    fake_bundle_with_device_page_content()
        .replace(
            r#"rect.setAttribute("width", "100");"#,
            r#"rect.setAttribute("width", "300");"#,
        )
        .replace(
            r#"rect.setAttribute("height", "60");"#,
            r#"rect.setAttribute("height", "200");"#,
        )
}
