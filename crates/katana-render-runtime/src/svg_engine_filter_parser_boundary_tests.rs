use super::usvg;
use rstest::rstest;

const FILTER_REGION: &str = "filterUnits='userSpaceOnUse' x='0' y='0' width='16' height='16'";
const IDENTITY_SATURATION: f32 = 1.0;
const IDENTITY_HUE_ROTATION: f32 = 0.0;
const AUTHORED_RED_SLOPE: f32 = 2.0;
const UNSUPPORTED_CHILD_SLOPE: f32 = 7.0;
const ZERO_INTERCEPT: f32 = 0.0;
const EMPTY_GROUP_FILTER_SVG: &str = r##"<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><g id='empty' filter='blur(2px)'/><g id='bounded' filter='blur(2px)'><rect width='8' height='8'/></g></svg>"##;
const EMBEDDED_SVG_IMAGE_MARKUP: &str = "<image id='source' width='1' height='1' href='data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxIiBoZWlnaHQ9IjEiPjxyZWN0IHdpZHRoPSIxIiBoZWlnaHQ9IjEiIGZpbGw9InJlZCIvPjwvc3ZnPg=='/>";

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
    let primitive = tree
        .filters()
        .first()
        .and_then(|filter| filter.primitives().first())
        .ok_or("flood primitive missing")?;
    assert_eq!(tree.filters()[0].primitives().len(), 1);
    assert!(matches!(primitive.kind(), usvg::filter::Kind::Flood(_)));
    Ok(())
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
#[case::path("<rect id='source' opacity='0.5' width='1' height='1'/>", "path")]
#[case::image(EMBEDDED_SVG_IMAGE_MARKUP, "image")]
#[case::text(
    "<text id='source' opacity='0.5' x='0' y='8' font-family='Noto Sans' font-size='8'>A</text>",
    "text"
)]
fn linked_feimage_moves_supported_child_ids_to_its_wrapper(
    #[case] child_markup: &str,
    #[case] child_kind: &str,
) -> Result<(), String> {
    let source = linked_feimage_svg(child_markup);
    let tree = parse_svg_with_writer_fonts(&source)?;
    assert_feimage_child(&tree, child_kind)
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

fn parse_filter(primitive: &str) -> Result<usvg::Tree, String> {
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><defs><filter id='f' {FILTER_REGION}>{primitive}</filter></defs><rect x='2' y='2' width='8' height='8' filter='url(#f)'/></svg>"
    );
    parse_svg(&source)
}

fn parse_svg(source: &str) -> Result<usvg::Tree, String> {
    static LOGGER: std::sync::Once = std::sync::Once::new();
    LOGGER.call_once(|| {
        if let Err(error) = env_logger::Builder::new()
            .is_test(true)
            .filter_level(log::LevelFilter::Warn)
            .try_init()
        {
            assert!(log::log_enabled!(log::Level::Warn), "{error}");
        }
    });
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

fn linked_feimage_svg(target_markup: &str) -> String {
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><defs><filter id='f' filterUnits='userSpaceOnUse' x='0' y='0' width='16' height='16'><feImage href='#source'/></filter></defs>{target_markup}<rect id='sample' x='1' y='1' width='8' height='8' filter='url(#f)'/></svg>"
    )
}

fn assert_feimage_child(tree: &usvg::Tree, child_kind: &str) -> Result<(), String> {
    let primitive = tree
        .filters()
        .first()
        .and_then(|filter| filter.primitives().first())
        .ok_or("feImage primitive missing")?;
    let usvg::filter::Kind::Image(image) = primitive.kind() else {
        return Err("feImage image data missing".to_owned());
    };
    let wrapper = first_group(image.root())?;
    assert_eq!(wrapper.id(), "source");
    let child = wrapper.children().first().ok_or("wrapped child missing")?;
    assert_eq!(child.id(), "");
    assert_image_child_kind(child, child_kind)
}

fn first_group(group: &usvg::Group) -> Result<&usvg::Group, String> {
    match group.children().first() {
        Some(usvg::Node::Group(child)) => Ok(child),
        _ => Err("feImage wrapper group missing".to_owned()),
    }
}

fn assert_image_child_kind(child: &usvg::Node, expected: &str) -> Result<(), String> {
    let matches = matches!(
        (expected, child),
        ("path", usvg::Node::Path(_))
            | ("image", usvg::Node::Image(_))
            | ("text", usvg::Node::Text(_))
    );
    if !matches {
        return Err(format!("feImage child kind mismatch: {expected}"));
    }
    Ok(())
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
