use super::DrawioJsRuntimeOps;
use super::test_support::fake_pixel_svg_bundle;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_limits_aws_crisp_translation_to_supported_non_edge_shapes() {
    let path = temp_runtime_path("krr-drawio-aws-crisp-geometry-unit");
    assert!(std::fs::write(&path, fake_bundle_with_aws_geometry()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render(aws_geometry_source(), &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"data-kind="aws" transform="translate(0.5,0.5)""#)
                && svg.contains(r#"data-kind="plain" transform="translate(0.5,0.5)""#)
        }),
        "{rendered:?}"
    );
    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"data-kind="other"><rect"#)
                && svg.contains(r#"data-kind="even"><rect"#)
                && svg.contains(r#"data-kind="edge"><path"#)
                && !svg.contains(r#"data-kind="other" transform="#)
                && !svg.contains(r#"data-kind="even" transform="#)
                && !svg.contains(r#"data-kind="edge" transform="#)
        }),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn aws_geometry_source() -> &'static str {
    r#"<mxGraphModel page="0"><root>
<mxCell id="1" parent="0"/>
<mxCell id="aws" style="shape=mxgraph.aws4.resourceIcon;" parent="1" vertex="1"><mxGeometry x="10" y="10" width="20" height="20" as="geometry"/></mxCell>
<mxCell id="plain" style="rounded=0;" parent="1" vertex="1"><mxGeometry x="40" y="10" width="20" height="20" as="geometry"/></mxCell>
<mxCell id="even" style="rounded=1;strokeWidth=2;" parent="1" vertex="1"><mxGeometry x="40" y="10" width="20" height="20" as="geometry"/></mxCell>
<mxCell id="other" style="shape=mxgraph.citrix.server;" parent="1" vertex="1"><mxGeometry x="70" y="10" width="20" height="20" as="geometry"/></mxCell>
<mxCell id="edge" style="endArrow=none;" parent="1" edge="1"><mxGeometry relative="1" as="geometry"/></mxCell>
</root></mxGraphModel>"#
}

fn fake_bundle_with_aws_geometry() -> &'static str {
    static BUNDLE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BUNDLE
        .get_or_init(|| fake_pixel_svg_bundle("100", "50", AWS_GEOMETRY_BODY, AWS_GEOMETRY_TAIL))
        .as_str()
}

const AWS_GEOMETRY_BODY: &str = r#"  svg.appendChild(createCellGroup("aws", "aws", "rect", 10));
  svg.appendChild(createCellGroup("plain", "plain", "rect", 40));
  svg.appendChild(createCellGroup("even", "even", "rect", 40));
  svg.appendChild(createCellGroup("other", "other", "rect", 70));
  svg.appendChild(createCellGroup("edge", "edge", "path", 10));
"#;

const AWS_GEOMETRY_TAIL: &str = r#"function createCellGroup(id, kind, tagName, x) {
  const outer = document.createElementNS("http://www.w3.org/2000/svg", "g");
  outer.setAttribute("data-cell-id", id);
  const inner = document.createElementNS("http://www.w3.org/2000/svg", "g");
  inner.setAttribute("data-kind", kind);
  const shape = document.createElementNS("http://www.w3.org/2000/svg", tagName);
  if (tagName === "path") {
    shape.setAttribute("d", "M 10 40 L 90 40");
  } else {
    shape.setAttribute("x", String(x));
    shape.setAttribute("y", "10");
    shape.setAttribute("width", "20");
    shape.setAttribute("height", "20");
  }
  inner.appendChild(shape);
  outer.appendChild(inner);
  return outer;
}
"#;
