use super::super::{DiagramRuntimeScript, DiagramV8Runtime};
use super::transformed_box;

#[path = "diagram_geometry_bridge_v8_boundary_tests.rs"]
mod v8_boundary_tests;

const BOUNDS_TOLERANCE: f64 = 1e-10;

#[cfg(target_os = "macos")]
#[test]
fn skia_svg_arc_path_reports_tight_bounds_for_fractional_geometry() -> Result<(), String> {
    let source = "M0,0 a10.209112313464553,43.139366149902344 0 0 1 0,-86.27873229980469 l226.20911231346454,0 a10.209112313464553,43.139366149902344 0 0 1 0,86.27873229980469";
    let path = skia_safe::Path::from_svg(source).ok_or("Skia rejected valid SVG arc")?;
    let bounds = path.compute_tight_bounds();
    assert!(bounds.is_finite());
    assert!(
        bounds.width() > 246.0 && bounds.width() < 247.0,
        "{bounds:?}"
    );
    assert!((bounds.height() - 86.27873).abs() < 0.001, "{bounds:?}");
    println!(
        "skia_fractional_arc_bounds={bounds:?} width_bits={:08x}",
        bounds.width().to_bits()
    );
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn skia_svg_full_fractional_arc_path_reports_bounds() -> Result<(), String> {
    let source = "M0,0 a10.209112313464553,43.139366149902344 0,0,1 0,-86.27873229980469 l226.20911231346454,0 a10.209112313464553,43.139366149902344 0,0,1 0,86.27873229980469 M226.20911231346454,-86.27873229980469 a10.209112313464553,43.139366149902344 0,0,0 0,86.27873229980469 l-226.20911231346454,0";
    let path = skia_safe::Path::from_svg(source).ok_or("Skia rejected valid SVG arc")?;
    let bounds = path.compute_tight_bounds();
    assert!(bounds.is_finite() && bounds.width() > 246.0 && bounds.width() < 247.0);
    println!(
        "skia_full_fractional_arc_bounds={bounds:?} width_bits={:08x}",
        bounds.width().to_bits()
    );
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn v8_svg_path_bounds_bridge_handles_valid_and_unsupported_paths() -> Result<(), String> {
    let script = r#"(() => {
      const bounds = globalThis.__katanaSvgPathBounds('M1,2 L11,2 L11,7 Z');
      const invalid = globalThis.__katanaSvgPathBounds('not a path');
      const empty = globalThis.__katanaSvgPathBounds('');
      const nul = globalThis.__katanaSvgPathBounds('M0,0\0L1,1');
      return JSON.stringify({bounds, invalid, empty, nul});
    })()"#;
    let output =
        DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed("path-bounds", script)])?;
    let result: serde_json::Value =
        serde_json::from_str(&output).map_err(|error| error.to_string())?;
    assert_eq!(
        result,
        serde_json::json!({"bounds":{"x":1,"y":2,"width":10,"height":5},"invalid":null,"empty":null,"nul":null})
    );
    Ok(())
}

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
fn v8_transform_callback_uses_own_fields_despite_prototype_setter() -> Result<(), String> {
    let script = r#"(() => {
        const previous = Object.getOwnPropertyDescriptor(Object.prototype, 'x');
        let intercepted = 0;
        Object.defineProperty(Object.prototype, 'x', {
            configurable: true,
            set(_) { intercepted += 1; },
        });
        try {
            const bounds = globalThis.__katanaTransformSvgBox('translate(10,20)', 1, 2, 3, 4);
            return [Object.hasOwn(bounds, 'x'), bounds.x, bounds.y,
                    bounds.width, bounds.height, intercepted].join(',');
        } finally {
            if (previous) Object.defineProperty(Object.prototype, 'x', previous);
            else delete Object.prototype.x;
        }
    })()"#;
    let result = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "geometry-bridge-prototype-setter",
        script,
    )])?;
    assert_eq!(result, "true,11,22,3,4,0");
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

#[test]
fn frozen_target_rejects_transformed_box_properties() {
    DiagramV8Runtime::ensure_initialized();
    DiagramV8Runtime::with_empty_try_catch(|scope| {
        let object = v8::Object::new(scope);
        assert_eq!(
            object.set_integrity_level(scope, v8::IntegrityLevel::Frozen),
            Some(true)
        );
        let bounds = super::TransformedBox {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        };
        assert_eq!(
            super::set_box_properties(scope, object, bounds),
            Err("SVG transformed bounds property write failed".to_owned())
        );
    });
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
