use super::usvg;
use rstest::rstest;

const FILTER_REGION: &str = "filterUnits='userSpaceOnUse' x='0' y='0' width='16' height='16'";
const IDENTITY_SATURATION: f32 = 1.0;
const IDENTITY_HUE_ROTATION: f32 = 0.0;
const AUTHORED_RED_SLOPE: f32 = 2.0;
const UNSUPPORTED_CHILD_SLOPE: f32 = 7.0;
const ZERO_INTERCEPT: f32 = 0.0;

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
    usvg::Tree::from_str(&source, &usvg::Options::default()).map_err(|error| error.to_string())
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
