use super::super as transfer;
use super::super::super::tests::{FailingTextRasterizer, TEXT_RASTERIZER_FAILURE};
use crate::markdown::svg_rasterize::html_glyph::HtmlGlyphRasterizer;
use resvg::usvg::{Group, Node, Tree, filter::TransferFunction};
use tiny_skia::{Pixmap, Transform};

#[test]
fn component_transfer_discards_partial_layer_when_text_rendering_fails() -> Result<(), String> {
    let svg =
        super::component_transfer_svg(super::LINEAR_FUNCTIONS, super::FILTER_REGION, "", "", "")
            .replace(
                "<text id=",
                "<rect width='10' height='10' fill='red'/><text id=",
            );
    let tree = parse(&svg)?;
    let group = filtered_group(&tree)?;
    assert!(matches!(group.children().first(), Some(Node::Path(_))));
    let mut target = Pixmap::new(super::SIZE, super::SIZE).ok_or("transfer canvas missing")?;
    target.fill(tiny_skia::Color::WHITE);
    let before = target.data().to_vec();
    let rasterizer = FailingTextRasterizer::default();
    let result = transfer::render(
        group,
        Transform::identity(),
        &mut target.as_mut(),
        &super::AuthoredClipInfo::default(),
        &rasterizer,
    );
    assert_eq!(result.err().as_deref(), Some(TEXT_RASTERIZER_FAILURE));
    assert_eq!(rasterizer.calls(), 1);
    assert_eq!(target.data(), before);
    Ok(())
}

#[test]
fn empty_component_transfer_layer_fails_before_rendering_children() -> Result<(), String> {
    use crate::markdown::svg_rasterize::html_shadow::Crop;

    let svg =
        super::component_transfer_svg(super::LINEAR_FUNCTIONS, super::FILTER_REGION, "", "", "");
    let tree = parse(&svg)?;
    let group = filtered_group(&tree)?;
    let rasterizer = FailingTextRasterizer::default();
    for (right, bottom) in [(0, 1), (1, 0)] {
        let result = transfer::render_source(
            group,
            Transform::identity(),
            Crop {
                left: 0,
                top: 0,
                right,
                bottom,
            },
            &super::AuthoredClipInfo::default(),
            &rasterizer,
        );
        assert_eq!(
            result.err().as_deref(),
            Some("component-transfer source allocation failed")
        );
    }
    assert_eq!(rasterizer.calls(), 0);
    Ok(())
}

fn identity_transfer_spec(group: &Group) -> Result<Option<transfer::TransferSpec>, String> {
    let identity = transfer::FilterTransform::new(Transform::identity())
        .ok_or("identity transform rejected")?;
    transfer::transfer_spec(group, identity)
}

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
fn svg_component_transfer_rejects_coefficients_outside_f32_range() -> Result<(), String> {
    const ORIGINAL_COEFFICIENTS: &str = "slope=\"1.2\" intercept=\"0\"";
    const ERROR: &str = "non-finite component-transfer function";
    for attributes in [
        r#"slope="1e39" intercept="0""#,
        r#"slope="1.2" intercept="-1e39""#,
    ] {
        let functions = super::LINEAR_FUNCTIONS.replacen(ORIGINAL_COEFFICIENTS, attributes, 1);
        let svg = super::component_transfer_svg(&functions, super::FILTER_REGION, "", "", "");
        let tree = parse(&svg)?;
        let group = filtered_group(&tree)?;
        assert_eq!(identity_transfer_spec(group).err().as_deref(), Some(ERROR));
    }
    Ok(())
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
        assert!(identity_transfer_spec(filtered_group(&tree)?)?.is_none());
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
        assert!(identity_transfer_spec(filtered_group(&tree)?)?.is_none());
    }
    Ok(())
}

#[test]
fn component_transfer_rejects_unfiltered_group_and_non_transfer_primitive() -> Result<(), String> {
    let unfiltered = parse(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="5" height="5"/></svg>"#,
    )?;
    assert!(transfer::supported_filter(unfiltered.root()).is_none());

    let tree = parse(&filtered_svg(r#"<feGaussianBlur stdDeviation="1"/>"#))?;
    let filter =
        transfer::supported_filter(filtered_group(&tree)?).ok_or("filter fixture missing")?;
    assert!(transfer::supported_primitive(filter).is_none());
    Ok(())
}

#[test]
fn component_transfer_disjoint_source_keeps_sibling_pixels() -> Result<(), String> {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><filter id="f" filterUnits="userSpaceOnUse" x="0" y="0" width="10" height="10" color-interpolation-filters="sRGB"><feComponentTransfer>{}</feComponentTransfer></filter></defs><rect x="12" y="12" width="5" height="5" fill="#ff0000"/><g filter="url(#f)"><rect x="25" y="25" width="5" height="5" fill="#00ff00"/></g></svg>"##,
        super::LINEAR_FUNCTIONS
    );
    let (pixels, _) = super::render_svg(&svg)?;
    let size = super::SIZE as usize;
    assert_eq!(pixels.len(), size * size);
    for (index, pixel) in pixels.iter().enumerate() {
        let x = index % size;
        let y = index / size;
        let expected = if (12..17).contains(&x) && (12..17).contains(&y) {
            [255, 0, 0, 255]
        } else {
            [0, 0, 0, 0]
        };
        assert_eq!(*pixel, expected, "pixel ({x}, {y})");
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

#[test]
fn disjoint_filter_and_primitive_regions_decline_transfer_without_drawing() -> Result<(), String> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><filter id="f" filterUnits="userSpaceOnUse" primitiveUnits="userSpaceOnUse" x="0" y="0" width="10" height="10" color-interpolation-filters="sRGB"><feComponentTransfer x="20" y="20" width="5" height="5">{}</feComponentTransfer></filter></defs><g filter="url(#f)"><rect width="30" height="30"/></g></svg>"#,
        super::LINEAR_FUNCTIONS
    );
    let tree = parse(&svg)?;
    let group = filtered_group(&tree)?;
    let filter = transfer::supported_filter(group).ok_or("filter missing")?;
    let [primitive] = filter.primitives() else {
        return Err("expected one component-transfer primitive".into());
    };
    assert!(filter.rect().right() < primitive.rect().x());
    assert!(filter.rect().bottom() < primitive.rect().y());
    assert!(identity_transfer_spec(group)?.is_none());
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let mut target = Pixmap::new(super::SIZE, super::SIZE).ok_or("transfer canvas missing")?;
    let before = target.data().to_vec();
    assert!(!transfer::render(
        group,
        Transform::identity(),
        &mut target.as_mut(),
        &super::AuthoredClipInfo::default(),
        &rasterizer,
    )?);
    assert_eq!(target.data(), before);
    Ok(())
}

#[test]
fn component_transfer_rejects_overflow_in_either_region_without_painting() -> Result<(), String> {
    let cases = [("40", "0", "1", true), ("1", "4", "5", false)];
    for (filter_width, primitive_x, primitive_width, filter_overflows) in cases {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><filter id="f" filterUnits="userSpaceOnUse" primitiveUnits="userSpaceOnUse" x="0" y="0" width="{filter_width}" height="10" color-interpolation-filters="sRGB"><feComponentTransfer x="{primitive_x}" y="0" width="{primitive_width}" height="5">{}</feComponentTransfer></filter></defs><g filter="url(#f)"><rect width="30" height="30"/></g></svg>"#,
            super::LINEAR_FUNCTIONS
        );
        assert_transfer_region_overflow(&parse(&svg)?, filter_overflows)?;
    }
    Ok(())
}

fn assert_transfer_region_overflow(tree: &Tree, filter_overflows: bool) -> Result<(), String> {
    let group = filtered_group(tree)?;
    let filter = transfer::supported_filter(group).ok_or("filter missing")?;
    let (primitive, _) = transfer::supported_primitive(filter).ok_or("primitive missing")?;
    let transform = Transform::from_scale(f32::MAX, 1.0);
    let filter_transform = transfer::FilterTransform::new(transform).ok_or("invalid scale")?;
    let map_rect = super::super::super::super::html_shadow::map_rect;
    assert_eq!(
        map_rect(filter.rect().to_rect(), filter_transform).is_err(),
        filter_overflows
    );
    assert_eq!(
        map_rect(primitive.rect().to_rect(), filter_transform).is_err(),
        !filter_overflows
    );
    let mut target = Pixmap::new(super::SIZE, super::SIZE).ok_or("transfer canvas missing")?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let result = transfer::render(
        group,
        transform,
        &mut target.as_mut(),
        &super::AuthoredClipInfo::default(),
        &rasterizer,
    );
    const ERROR: &str = "drop-shadow transformed bounds are non-finite";
    assert_eq!(result.err().as_deref(), Some(ERROR));
    assert!(target.data().iter().all(|byte| *byte == 0));
    Ok(())
}
