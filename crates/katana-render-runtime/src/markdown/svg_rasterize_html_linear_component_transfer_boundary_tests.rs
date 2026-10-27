use super::super as transfer;
use crate::markdown::svg_rasterize::html_glyph::HtmlGlyphRasterizer;
use resvg::usvg::{Group, Node, Tree, filter::TransferFunction};
use tiny_skia::{Pixmap, Transform};

#[test]
fn component_transfer_rejects_nonfinite_linear_coefficients() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for (slope, intercept) in [(value, 0.0), (1.0, value)] {
            let result = transfer::linear(&TransferFunction::Linear { slope, intercept });
            assert!(result.is_err_and(|error| error.contains("non-finite component-transfer")));
        }
    }
    assert_eq!(transfer::linear(&TransferFunction::Identity), Ok(None));
}

#[test]
fn component_transfer_rejects_non_linear_green_and_blue_channels() -> Result<(), String> {
    for channel in ["G", "B"] {
        let functions = super::LINEAR_FUNCTIONS.replace(
            &format!(r#"<feFunc{channel} type="linear" slope="1.2" intercept="0"/>"#),
            &format!(r#"<feFunc{channel} type="table" tableValues="0 1"/>"#),
        );
        let svg = super::component_transfer_svg(&functions, super::FILTER_REGION, "", "", "");
        let tree = parse(&svg)?;
        assert!(transfer::transfer_spec(filtered_group(&tree)?, Transform::identity())?.is_none());
    }
    Ok(())
}

#[test]
fn component_transfer_declines_unsupported_inputs_and_primitive_shapes() -> Result<(), String> {
    let cases = [
        r#"<feComponentTransfer in="SourceAlpha"/>"#,
        r#"<feGaussianBlur stdDeviation="1"/>"#,
        r#"<feOffset dx="1"/><feComponentTransfer/>"#,
    ];
    for primitive in cases {
        let svg = filtered_svg(primitive);
        let tree = parse(&svg)?;
        assert!(transfer::transfer_spec(filtered_group(&tree)?, Transform::identity())?.is_none());
    }
    Ok(())
}

#[test]
fn component_transfer_declines_unsupported_and_off_canvas_transforms() -> Result<(), String> {
    let svg =
        super::component_transfer_svg(super::LINEAR_FUNCTIONS, super::FILTER_REGION, "", "", "");
    let tree = parse(&svg)?;
    let group = filtered_group(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let mut target = Pixmap::new(super::SIZE, super::SIZE).ok_or("transfer canvas missing")?;
    for transform in [
        Transform::from_rotate(30.0),
        Transform::from_translate(1024.0, 1024.0),
    ] {
        assert!(!transfer::render(
            group,
            transform,
            &mut target.as_mut(),
            &super::AuthoredClipInfo::default(),
            &rasterizer,
        )?);
    }
    assert!(target.data().iter().all(|byte| *byte == 0));
    Ok(())
}

fn filtered_svg(primitive: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><filter id="f" filterUnits="userSpaceOnUse" x="0" y="0" width="40" height="40" color-interpolation-filters="sRGB">{primitive}</filter></defs><g filter="url(#f)"><rect width="20" height="20"/></g></svg>"#
    )
}

fn parse(svg: &str) -> Result<Tree, String> {
    let options = super::super::super::super::rasterizer_options();
    Tree::from_str(svg, &options).map_err(|error| error.to_string())
}

fn filtered_group(tree: &Tree) -> Result<&Group, String> {
    fn find(group: &Group) -> Option<&Group> {
        if !group.filters().is_empty() {
            return Some(group);
        }
        group.children().iter().find_map(|node| match node {
            Node::Group(child) => find(child),
            _ => None,
        })
    }
    find(tree.root()).ok_or_else(|| "filtered fixture group is missing".to_owned())
}
