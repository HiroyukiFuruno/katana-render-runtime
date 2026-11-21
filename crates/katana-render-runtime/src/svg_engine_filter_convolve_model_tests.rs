use super::{fixture, usvg};
use rstest::rstest;

const DEFAULT_KERNEL: &str = "<feConvolveMatrix kernelMatrix='0 1 0 1 4 1 0 1 0'/>";
const TWO_BY_ONE_KERNEL: &str = "<feConvolveMatrix order='2 1' kernelMatrix='1 2' targetX='0' divisor='3' bias='0.25' edgeMode='wrap' preserveAlpha='true'/>";
const ONE_ORDER_KERNEL: &str = "<feConvolveMatrix order='2' kernelMatrix='1 2 3 4'/>";
const ZERO_SUM_KERNEL: &str = "<feConvolveMatrix order='2 1' kernelMatrix='1 -1'/>";
const DEFAULT_DIMENSIONS: (u32, u32) = (3, 3);
const DEFAULT_TARGET: (u32, u32) = (1, 1);
const DEFAULT_DATA: &[f32] = &[0.0, 1.0, 0.0, 1.0, 4.0, 1.0, 0.0, 1.0, 0.0];
const DEFAULT_DIVISOR: f32 = 8.0;
const DEFAULT_BIAS: f32 = 0.0;
const TWO_BY_ONE_DIMENSIONS: (u32, u32) = (2, 1);
const TWO_BY_ONE_TARGET: (u32, u32) = (0, 0);
const TWO_BY_ONE_DATA: &[f32] = &[1.0, 2.0];
const EXPLICIT_DIVISOR: f32 = 3.0;
const EXPLICIT_BIAS: f32 = 0.25;
const ONE_ORDER_DIMENSIONS: (u32, u32) = (2, 2);
const ONE_ORDER_TARGET: (u32, u32) = (1, 1);
const ONE_ORDER_DATA: &[f32] = &[1.0, 2.0, 3.0, 4.0];
const ONE_ORDER_DIVISOR: f32 = 10.0;
const ZERO_SUM_TARGET: (u32, u32) = (1, 0);
const ZERO_SUM_DATA: &[f32] = &[1.0, -1.0];
const DEFAULT_ZERO_SUM_DIVISOR: f32 = 1.0;
const PRECEDING_RESULT: &str = "visible";
const INVALID_DIVISOR: &str =
    "<feFlood result='visible'/><feConvolveMatrix order='2 1' kernelMatrix='1 2' divisor='0'/>";
const INVALID_TARGET: &str =
    "<feFlood result='visible'/><feConvolveMatrix order='2 1' kernelMatrix='1 2' targetX='2'/>";
const INVALID_TARGET_Y: &str =
    "<feFlood result='visible'/><feConvolveMatrix order='2 1' kernelMatrix='1 2' targetY='1'/>";
const INVALID_KERNEL_LENGTH: &str =
    "<feFlood result='visible'/><feConvolveMatrix order='2 1' kernelMatrix='1 2 3'/>";

#[derive(Clone, Copy)]
struct ExpectedConvolve {
    dimensions: (u32, u32),
    target: (u32, u32),
    data: &'static [f32],
    divisor: f32,
    bias: f32,
    edge_mode: usvg::filter::EdgeMode,
    preserve_alpha: bool,
}

const DEFAULT_EXPECTED: ExpectedConvolve = ExpectedConvolve {
    dimensions: DEFAULT_DIMENSIONS,
    target: DEFAULT_TARGET,
    data: DEFAULT_DATA,
    divisor: DEFAULT_DIVISOR,
    bias: DEFAULT_BIAS,
    edge_mode: usvg::filter::EdgeMode::Duplicate,
    preserve_alpha: false,
};
const TWO_BY_ONE_EXPECTED: ExpectedConvolve = ExpectedConvolve {
    dimensions: TWO_BY_ONE_DIMENSIONS,
    target: TWO_BY_ONE_TARGET,
    data: TWO_BY_ONE_DATA,
    divisor: EXPLICIT_DIVISOR,
    bias: EXPLICIT_BIAS,
    edge_mode: usvg::filter::EdgeMode::Wrap,
    preserve_alpha: true,
};
const ONE_ORDER_EXPECTED: ExpectedConvolve = ExpectedConvolve {
    dimensions: ONE_ORDER_DIMENSIONS,
    target: ONE_ORDER_TARGET,
    data: ONE_ORDER_DATA,
    divisor: ONE_ORDER_DIVISOR,
    bias: DEFAULT_BIAS,
    edge_mode: usvg::filter::EdgeMode::Duplicate,
    preserve_alpha: false,
};
const ZERO_SUM_EXPECTED: ExpectedConvolve = ExpectedConvolve {
    dimensions: TWO_BY_ONE_DIMENSIONS,
    target: ZERO_SUM_TARGET,
    data: ZERO_SUM_DATA,
    divisor: DEFAULT_ZERO_SUM_DIVISOR,
    bias: DEFAULT_BIAS,
    edge_mode: usvg::filter::EdgeMode::Duplicate,
    preserve_alpha: false,
};

#[rstest]
#[case::default_order(DEFAULT_KERNEL, DEFAULT_EXPECTED)]
#[case::two_orders_and_options(TWO_BY_ONE_KERNEL, TWO_BY_ONE_EXPECTED)]
#[case::single_order_repeats_for_both_axes(ONE_ORDER_KERNEL, ONE_ORDER_EXPECTED)]
#[case::zero_sum_defaults_divisor(ZERO_SUM_KERNEL, ZERO_SUM_EXPECTED)]
fn convolve_matrix_values_follow_authored_kernel_and_defaults(
    #[case] primitive: &str,
    #[case] expected: ExpectedConvolve,
) -> Result<(), String> {
    let tree = fixture(primitive)?;
    let convolve = convolve_primitive(&tree)?;
    assert_convolve_values(convolve, expected);
    Ok(())
}

fn convolve_primitive(tree: &usvg::Tree) -> Result<&usvg::filter::ConvolveMatrix, String> {
    let primitive = tree
        .filters()
        .first()
        .and_then(|filter| filter.primitives().first())
        .ok_or("convolve primitive missing")?;
    let usvg::filter::Kind::ConvolveMatrix(value) = primitive.kind() else {
        return Err("expected feConvolveMatrix".to_owned());
    };
    Ok(value)
}

fn assert_convolve_values(value: &usvg::filter::ConvolveMatrix, expected: ExpectedConvolve) {
    let matrix = value.matrix();
    assert_eq!((matrix.columns(), matrix.rows()), expected.dimensions);
    assert_eq!((matrix.target_x(), matrix.target_y()), expected.target);
    assert_eq!(matrix.data(), expected.data);
    assert_eq!(value.divisor().get(), expected.divisor);
    assert_eq!(value.bias(), expected.bias);
    assert_eq!(value.edge_mode(), expected.edge_mode);
    assert_eq!(value.preserve_alpha(), expected.preserve_alpha);
}

#[rstest]
#[case::zero_divisor(INVALID_DIVISOR)]
#[case::out_of_bounds_target(INVALID_TARGET)]
#[case::out_of_bounds_target_y(INVALID_TARGET_Y)]
#[case::wrong_kernel_length(INVALID_KERNEL_LENGTH)]
fn invalid_convolve_matrix_becomes_transparent_dummy_after_prior_primitive(
    #[case] children: &str,
) -> Result<(), String> {
    let tree = fixture(children)?;
    let primitives = tree.filters().first().ok_or("filter missing")?.primitives();
    assert_eq!(primitives.len(), 2);
    assert_eq!(primitives[0].result(), PRECEDING_RESULT);
    assert_dummy_flood(&primitives[1])?;
    Ok(())
}

fn assert_dummy_flood(primitive: &usvg::filter::Primitive) -> Result<(), String> {
    let usvg::filter::Kind::Flood(flood) = primitive.kind() else {
        return Err("invalid convolve matrix was not replaced with a dummy flood".to_owned());
    };
    assert_eq!(flood.color(), usvg::Color::black());
    assert_eq!(flood.opacity(), usvg::Opacity::ZERO);
    Ok(())
}
