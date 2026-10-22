use super::super::{DiagramRuntimeScript, DiagramV8Runtime};
use super::transformed_box;

const BOUNDS_TOLERANCE: f64 = 1e-10;

const V8_BOUNDARY_CASES_SCRIPT: &str = r#"(() => {
    const transform = globalThis.__katanaTransformSvgBox;
    const cases = [
        ['missing', () => transform('translate(1)', 0, 0)],
        ['nan', () => transform('translate(1)', NaN, 0, 1, 1)],
        ['infinite', () => transform('translate(1)', 0, 0, Infinity, 1)],
        ['negative', () => transform('translate(1)', 0, 0, -1, 1)],
        ['malformed-transform', () => transform('rotate(nope)', 2, 3, 4, 5)],
    ];
    return cases.map(([name, call]) => {
        try {
            const box = call();
            return `${name}:${box.x},${box.y},${box.width},${box.height}`;
        } catch (error) {
            return `${name}:error:${error.message}`;
        }
    }).join('|');
})()"#;

#[test]
fn svg_rotation_pivot_transforms_all_four_box_corners() {
    let bounds = transformed_box("rotate(90, 1, 1)", [0.0, 0.0, 2.0, 1.0]);

    assert_bounds(bounds, [1.0, 0.0, 1.0, 2.0]);
}

#[test]
fn svg_transform_list_preserves_library_order_and_scaling() {
    let bounds = transformed_box("translate(10) scale(2)", [0.0, 0.0, 2.0, 1.0]);

    assert_bounds(bounds, [10.0, 0.0, 4.0, 2.0]);
}

#[test]
fn svg_transform_ignores_malformed_syntax_but_rejects_non_finite_bounds() {
    assert_bounds(
        transformed_box("rotate(nope)", [0.0, 0.0, 1.0, 1.0]),
        [0.0, 0.0, 1.0, 1.0],
    );
    assert!(transformed_box("rotate(45)", [f64::NAN, 0.0, 1.0, 1.0]).is_err());
    assert!(transformed_box("scale(2)", [0.0, 0.0, -1.0, 1.0]).is_err());
}

#[test]
fn v8_transform_callback_returns_bounds_and_rejects_finite_input_overflow() -> Result<(), String> {
    let script = format!(
        r#"(() => {{
        const transform = globalThis.__katanaTransformSvgBox;
        const bounds = transform('rotate(90, 1, 1)', 0, 0, 2, 1);
        const keys = ['x', 'y', 'width', 'height'];
        const expected = [1, 0, 1, 2];
        const normal = keys.every((key, index) =>
            Math.abs(bounds[key] - expected[index]) < {BOUNDS_TOLERANCE});
        let overflow = '';
        try {{
            transform('translate(1e308, 0)', 1e308, 0, 1, 1);
        }} catch (error) {{
            overflow = error.message;
        }}
        return [normal, overflow].join(',');
    }})()"#
    );
    let result = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "geometry-bridge-overflow-boundary",
        &script,
    )])?;
    assert_eq!(result, "true,Non-finite SVG transformed bounds");
    Ok(())
}

#[test]
fn v8_transform_callback_handles_boundary_case_table() -> Result<(), String> {
    let result = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "geometry-bridge-boundary-cases",
        V8_BOUNDARY_CASES_SCRIPT,
    )])?;
    assert_eq!(
        result,
        "missing:error:Invalid SVG child bounds|nan:error:Invalid SVG child bounds|\
         infinite:error:Invalid SVG child bounds|negative:error:Invalid SVG child bounds|\
         malformed-transform:2,3,4,5"
    );
    Ok(())
}

fn assert_bounds(
    actual: Result<super::TransformedBox, String>,
    expected: [f64; super::BOUNDS_COMPONENT_COUNT],
) {
    let matches = actual.as_ref().is_ok_and(|bounds| {
        let actual = [bounds.x, bounds.y, bounds.width, bounds.height];
        (0..super::BOUNDS_COMPONENT_COUNT)
            .all(|index| (actual[index] - expected[index]).abs() < BOUNDS_TOLERANCE)
    });
    assert!(matches, "unexpected transformed bounds: {actual:?}");
}
