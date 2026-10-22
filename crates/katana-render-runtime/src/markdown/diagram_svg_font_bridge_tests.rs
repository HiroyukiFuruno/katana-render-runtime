use super::set_svg_metric_properties;
use super::set_svg_metric_property;
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
use crate::markdown::svg_rasterize::SvgTextRunMetrics;

const BASE_METRICS: SvgTextRunMetrics = SvgTextRunMetrics {
    advance: 11.5,
    ascent: 8.25,
    descent: 2.0,
    layout_ascent: None,
    ink_bounds: None,
};
const OPTIONAL_LAYOUT_ASCENT: f32 = 6.75;
const OPTIONAL_INK_BOUNDS: [f64; 4] = [1.0, 2.0, 3.0, 4.0];

const INVALID_X_HEIGHT_REQUEST_SCRIPT: &str = r#"(() => {
    const measure = globalThis.__katanaMeasureSvgFontXHeight;
    const cases = [
        ['size-zero', 0, 400], ['size-negative', -1, 400],
        ['size-nan', NaN, 400], ['size-infinity', Infinity, 400],
        ['size-min', Number.MIN_VALUE, 400], ['size-max', Number.MAX_VALUE, 400],
        ['weight-negative', 16, -1], ['weight-zero', 16, 0],
        ['weight-high', 16, 1001], ['weight-nan', 16, NaN],
        ['weight-infinity', 16, Infinity],
    ];
    return cases.map(([name, size, weight]) => {
        let rejected = false;
        try { measure('Noto Sans', size, weight, false); }
        catch (error) {
            rejected = error instanceof Error &&
                error.message === 'Invalid SVG font x-height request';
        }
        return [name, rejected].join(':');
    }).join(',');
})()"#;

fn metric_fixture(include_optional: bool) -> SvgTextRunMetrics {
    SvgTextRunMetrics {
        layout_ascent: include_optional.then_some(OPTIONAL_LAYOUT_ASCENT),
        ink_bounds: include_optional.then_some(OPTIONAL_INK_BOUNDS),
        ..BASE_METRICS
    }
}

fn assert_number_property(
    scope: &mut v8::PinScope<'_, '_>,
    object: v8::Local<'_, v8::Object>,
    name: &str,
    expected: f64,
) {
    let actual = v8::String::new(scope, name)
        .ok_or("property key allocation failed")
        .and_then(|key| {
            object
                .get(scope, key.into())
                .ok_or("property value lookup failed")
        })
        .map(|value| value.number_value(scope));
    assert_eq!(actual, Ok(Some(expected)), "property={name}");
}

fn assert_property_presence(
    scope: &mut v8::PinScope<'_, '_>,
    object: v8::Local<'_, v8::Object>,
    name: &str,
    expected: bool,
) {
    let actual = v8::String::new(scope, name)
        .ok_or("property key allocation failed")
        .and_then(|key| {
            object
                .has(scope, key.into())
                .ok_or("property presence lookup failed")
        });
    assert_eq!(actual, Ok(expected), "property={name}");
}

#[test]
fn frozen_v8_object_rejects_metric_property_write() {
    DiagramV8Runtime::ensure_initialized();
    DiagramV8Runtime::with_empty_try_catch(|scope| {
        let object = v8::Object::new(scope);
        assert_eq!(
            object.set_integrity_level(scope, v8::IntegrityLevel::Frozen),
            Some(true)
        );
        assert_eq!(
            set_svg_metric_property(scope, object, "advance", 1.0),
            Err("SVG font metric result allocation failed")
        );
    });
}

#[test]
fn metric_object_preserves_optional_property_contract() {
    DiagramV8Runtime::ensure_initialized();
    DiagramV8Runtime::with_empty_try_catch(|scope| {
        let without_optional = v8::Object::new(scope);
        assert_eq!(
            set_svg_metric_properties(scope, without_optional, metric_fixture(false)),
            Ok(())
        );
        assert_number_property(scope, without_optional, "advance", 11.5);
        assert_property_presence(scope, without_optional, "inkLeft", false);
        assert_property_presence(scope, without_optional, "layoutAscent", false);

        let with_optional = v8::Object::new(scope);
        assert_eq!(
            set_svg_metric_properties(scope, with_optional, metric_fixture(true)),
            Ok(())
        );
        assert_number_property(scope, with_optional, "layoutAscent", 6.75);
        assert_number_property(scope, with_optional, "inkRight", 3.0);
    });
}

#[test]
fn real_v8_x_height_bridge_rejects_invalid_numeric_requests() -> Result<(), String> {
    let observed = DiagramV8Runtime::render(&[
        crate::markdown::diagram_js_runtime::DiagramRuntimeScript::borrowed(
            "bridge-invalid-x-height-request-table",
            INVALID_X_HEIGHT_REQUEST_SCRIPT,
        ),
    ])?;
    let expected = [
        "size-zero",
        "size-negative",
        "size-nan",
        "size-infinity",
        "size-min",
        "size-max",
        "weight-negative",
        "weight-zero",
        "weight-high",
        "weight-nan",
        "weight-infinity",
    ]
    .into_iter()
    .map(|name| format!("{name}:true"))
    .collect::<Vec<_>>()
    .join(",");
    assert_eq!(observed, expected);
    Ok(())
}
