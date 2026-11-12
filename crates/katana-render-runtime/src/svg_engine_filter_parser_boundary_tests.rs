use super::usvg;
use rstest::rstest;

#[path = "svg_engine_filter_parser_feimage_tests.rs"]
mod feimage_tests;

const FILTER_REGION: &str = "filterUnits='userSpaceOnUse' x='0' y='0' width='16' height='16'";
const IDENTITY_SATURATION: f32 = 1.0;
const IDENTITY_HUE_ROTATION: f32 = 0.0;
const AUTHORED_RED_SLOPE: f32 = 2.0;
const UNSUPPORTED_CHILD_SLOPE: f32 = 7.0;
const ZERO_INTERCEPT: f32 = 0.0;
const EMPTY_GROUP_FILTER_SVG: &str = r##"<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><g id='empty' filter='blur(2px)'/><g id='bounded' filter='blur(2px)'><rect width='8' height='8'/></g></svg>"##;

#[rstest]
#[case::zero_filter_region(r##"<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><defs><filter id='bad' filterUnits='userSpaceOnUse' x='0' y='0' width='0' height='16'><feFlood/></filter></defs><rect id='subject' width='8' height='8' filter='url(#bad)'/><rect id='control' x='8' width='8' height='8'/></svg>"##)]
#[case::wrong_href_type(r##"<svg xmlns='http://www.w3.org/2000/svg' xmlns:xlink='http://www.w3.org/1999/xlink' width='16' height='16'><defs><linearGradient id='gradient'><stop/><stop offset='1'/></linearGradient><filter id='bad' xlink:href='#gradient'/></defs><rect id='subject' width='8' height='8' filter='url(#bad)'/><rect id='control' x='8' width='8' height='8'/></svg>"##)]
fn invalid_filter_definition_drops_only_its_filtered_shape(
    #[case] source: &str,
) -> Result<(), String> {
    let tree = parse_svg(source)?;
    assert!(tree.node_by_id("subject").is_none());
    assert!(tree.node_by_id("control").is_some());
    Ok(())
}

#[test]
fn filter_functions_without_object_bounds_skip_only_the_empty_group_filter() -> Result<(), String> {
    let tree = parse_svg(EMPTY_GROUP_FILTER_SVG)?;
    assert!(log::log_enabled!(log::Level::Warn));
    assert!(group_by_id(&tree, "empty")?.filters().is_empty());
    assert_eq!(group_by_id(&tree, "bounded")?.filters().len(), 1);
    Ok(())
}

#[test]
fn filter_children_skip_comments_and_non_primitive_elements() -> Result<(), String> {
    let tree = parse_filter("<!-- retained XML node --><feFlood/><rect width='1' height='1'/>")?;
    let kind = first_filter_kind(&tree)?;
    assert_eq!(tree.filters()[0].primitives().len(), 1);
    assert!(matches!(kind, usvg::filter::Kind::Flood(_)));
    Ok(())
}

#[test]
fn invalid_embedded_image_keeps_unfiltered_content_visible() -> Result<(), String> {
    for (href, valid_base64) in [
        ("data:image/png;base64,A", false),
        ("data:image/png;base64,iVBORw0KGgo=", true),
    ] {
        let url = data_url::DataUrl::process(href).map_err(|error| format!("{error:?}"))?;
        assert_eq!(url.decode_to_vec().is_ok(), valid_base64);
        let source = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><defs><filter id='f' {FILTER_REGION}><feImage href='{href}'/></filter></defs><image id='bad' width='1' height='1' href='{href}'/><rect width='8' height='8' filter='url(#f)'/><rect id='control' x='8' width='8' height='8'/></svg>"
        );
        let tree = parse_svg(&source)?;
        assert!(tree.node_by_id("bad").is_none());
        assert!(tree.node_by_id("control").is_some());
        assert_transparent_flood(first_filter_kind(&tree)?);
    }
    Ok(())
}

#[test]
fn specular_lighting_requires_a_light_source() -> Result<(), String> {
    let invalid = parse_filter("<feSpecularLighting/>")?;
    assert_transparent_flood(first_filter_kind(&invalid)?);
    let valid =
        parse_filter("<feSpecularLighting><fePointLight x='2' y='3' z='4'/></feSpecularLighting>")?;
    assert!(matches!(
        first_filter_kind(&valid)?,
        usvg::filter::Kind::SpecularLighting(_)
    ));
    Ok(())
}

fn first_filter_kind(tree: &usvg::Tree) -> Result<&usvg::filter::Kind, String> {
    tree.filters()
        .first()
        .and_then(|filter| filter.primitives().first())
        .map(usvg::filter::Primitive::kind)
        .ok_or_else(|| "filter primitive missing".to_owned())
}

fn assert_transparent_flood(kind: &usvg::filter::Kind) {
    assert!(matches!(kind, usvg::filter::Kind::Flood(flood) if flood.opacity().get() == 0.0));
}

#[rstest]
#[case::flood("<feFlood width='0' height='1'/>", "<feFlood width='1' height='1'/>")]
#[case::image(
    "<feImage href='#source' width='0' height='1'/>",
    "<feImage href='#source' width='1' height='1'/>"
)]
fn zero_object_bbox_primitive_subregion_drops_only_the_invalid_shape(
    #[case] invalid_primitive: &str,
    #[case] valid_primitive: &str,
) -> Result<(), String> {
    let invalid = parse_object_bbox_filter(invalid_primitive)?;
    assert!(invalid.node_by_id("subject").is_none());
    let valid = parse_object_bbox_filter(valid_primitive)?;
    assert!(valid.node_by_id("subject").is_some());
    Ok(())
}

#[rstest]
#[case::omitted_interpolation("")]
#[case::invalid_interpolation("color-interpolation-filters='invalid'")]
fn empty_saturate_values_use_the_identity_default(
    #[case] interpolation: &str,
) -> Result<(), String> {
    let tree = parse_filter(&format!(
        "<feColorMatrix type='saturate' values='' {interpolation}/>"
    ))?;
    let (kind, color_interpolation) = color_matrix(&tree)?;
    let usvg::filter::ColorMatrixKind::Saturate(amount) = kind else {
        return Err("saturate matrix missing".to_owned());
    };
    assert_eq!(amount.get(), IDENTITY_SATURATION);
    assert_eq!(
        color_interpolation,
        usvg::filter::ColorInterpolation::LinearRGB
    );
    Ok(())
}

#[test]
fn empty_hue_rotate_values_use_zero_and_linear_rgb_defaults() -> Result<(), String> {
    let tree = parse_filter("<feColorMatrix type='hueRotate' values='' />")?;
    let (kind, color_interpolation) = color_matrix(&tree)?;
    let usvg::filter::ColorMatrixKind::HueRotate(angle) = kind else {
        return Err("hue-rotate matrix missing".to_owned());
    };
    assert_eq!(*angle, IDENTITY_HUE_ROTATION);
    assert_eq!(
        color_interpolation,
        usvg::filter::ColorInterpolation::LinearRGB
    );
    Ok(())
}

#[test]
fn component_transfer_ignores_non_function_children() -> Result<(), String> {
    let tree = parse_filter(&format!(
        "<feComponentTransfer><feFuncR type='linear' slope='{AUTHORED_RED_SLOPE}'/><rect type='linear' slope='{UNSUPPORTED_CHILD_SLOPE}'/></feComponentTransfer>"
    ))?;
    let transfer = component_transfer(&tree)?;
    assert_linear(transfer.func_r(), AUTHORED_RED_SLOPE)?;
    assert_identity(transfer.func_g())?;
    assert_identity(transfer.func_b())?;
    assert_identity(transfer.func_a())
}

#[rstest]
#[case::single("0.125", 0.125, 0.125)]
#[case::paired("0.125 0.25", 0.125, 0.25)]
fn turbulence_frequency_preserves_one_or_two_svg_values(
    #[case] frequency: &str,
    #[case] expected_x: f32,
    #[case] expected_y: f32,
) -> Result<(), String> {
    let tree = parse_filter(&format!("<feTurbulence baseFrequency='{frequency}'/>"))?;
    let primitive = tree
        .filters()
        .first()
        .and_then(|filter| filter.primitives().first())
        .ok_or("turbulence primitive missing")?;
    let usvg::filter::Kind::Turbulence(turbulence) = primitive.kind() else {
        return Err("turbulence kind differs".to_owned());
    };
    assert_eq!(turbulence.base_frequency_x().get(), expected_x);
    assert_eq!(turbulence.base_frequency_y().get(), expected_y);
    Ok(())
}

#[rstest]
#[case::zero_seed("0", "1")]
#[case::upper_seed("2147483647", "2147483646")]
#[case::ordinary_seed("17", "17")]
fn stitched_turbulence_seed_boundaries_preserve_rendered_pixels(
    #[case] seed: &str,
    #[case] normalized_seed: &str,
) -> Result<(), String> {
    let render = |seed: &str| {
        super::fixture(&format!(
            "<feTurbulence type='fractalNoise' baseFrequency='1' numOctaves='1' seed='{seed}' stitchTiles='stitch'/>",
        ))
        .and_then(|tree| super::render(&tree))
    };
    let actual = render(seed)?;
    let expected = render(normalized_seed)?;

    assert_eq!(actual.data(), expected.data());
    assert!(actual.data().chunks_exact(4).any(|pixel| pixel[3] > 0));
    Ok(())
}

#[test]
fn stitched_turbulence_rounds_fractional_frequency_to_one_tile_cycle() -> Result<(), String> {
    let render_frequency = |frequency: &str| {
        super::fixture(&format!(
            "<feTurbulence type='fractalNoise' baseFrequency='{frequency} 0' numOctaves='2' seed='17' stitchTiles='stitch'/>"
        ))
        .and_then(|tree| super::render(&tree))
    };
    let fractional = render_frequency("0.05")?;
    let one_cycle = render_frequency("0.0625")?;
    assert!(one_cycle.pixels().iter().any(|pixel| pixel.alpha() > 0));
    assert_eq!(fractional.data(), one_cycle.data());
    Ok(())
}

fn parse_filter(primitive: &str) -> Result<usvg::Tree, String> {
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><defs><filter id='f' {FILTER_REGION}>{primitive}</filter></defs><rect x='2' y='2' width='8' height='8' filter='url(#f)'/></svg>"
    );
    parse_svg(&source)
}

fn parse_svg(source: &str) -> Result<usvg::Tree, String> {
    crate::svg_engine::init_test_logger();
    usvg::Tree::from_str(source, &usvg::Options::default()).map_err(|error| error.to_string())
}

fn parse_svg_with_writer_fonts(source: &str) -> Result<usvg::Tree, String> {
    let fontdb = super::super::writer_tests::upstream_writer_font_database()?;
    let options = usvg::Options {
        fontdb,
        ..usvg::Options::default()
    };
    usvg::Tree::from_str(source, &options).map_err(|error| error.to_string())
}

fn group_by_id<'a>(tree: &'a usvg::Tree, id: &str) -> Result<&'a usvg::Group, String> {
    match tree.node_by_id(id) {
        Some(usvg::Node::Group(group)) => Ok(group),
        _ => Err(format!("group {id} missing")),
    }
}

fn parse_object_bbox_filter(primitive: &str) -> Result<usvg::Tree, String> {
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><defs><filter id='f' filterUnits='userSpaceOnUse' primitiveUnits='objectBoundingBox' x='0' y='0' width='16' height='16'>{primitive}</filter></defs><rect id='subject' x='2' y='2' width='8' height='8' filter='url(#f)'/><rect id='source' x='1' y='1' width='1' height='1'/></svg>"
    );
    parse_svg(&source)
}

fn color_matrix(
    tree: &usvg::Tree,
) -> Result<
    (
        &usvg::filter::ColorMatrixKind,
        usvg::filter::ColorInterpolation,
    ),
    String,
> {
    let primitive = tree
        .filters()
        .first()
        .and_then(|filter| filter.primitives().first())
        .ok_or("color matrix missing")?;
    let usvg::filter::Kind::ColorMatrix(matrix) = primitive.kind() else {
        return Err("color matrix kind missing".to_owned());
    };
    Ok((matrix.kind(), primitive.color_interpolation()))
}

fn component_transfer(tree: &usvg::Tree) -> Result<&usvg::filter::ComponentTransfer, String> {
    let primitive = tree
        .filters()
        .first()
        .and_then(|filter| filter.primitives().first())
        .ok_or("component transfer missing")?;
    let usvg::filter::Kind::ComponentTransfer(transfer) = primitive.kind() else {
        return Err("component transfer kind missing".to_owned());
    };
    Ok(transfer)
}

fn assert_linear(
    function: &usvg::filter::TransferFunction,
    expected_slope: f32,
) -> Result<(), String> {
    match function {
        usvg::filter::TransferFunction::Linear { slope, intercept } => {
            assert_eq!(*slope, expected_slope);
            assert_eq!(*intercept, ZERO_INTERCEPT);
            Ok(())
        }
        _ => Err("linear transfer missing".to_owned()),
    }
}

fn assert_identity(function: &usvg::filter::TransferFunction) -> Result<(), String> {
    match function {
        usvg::filter::TransferFunction::Identity => Ok(()),
        _ => Err("identity transfer changed by non-function child".to_owned()),
    }
}
