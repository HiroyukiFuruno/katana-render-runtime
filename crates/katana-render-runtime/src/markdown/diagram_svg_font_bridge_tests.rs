use super::set_svg_metric_properties;
use super::set_svg_metric_property;
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
use crate::markdown::svg_rasterize::SvgTextRunMetrics;
use crate::v8_script::ScriptCompiler;

#[path = "diagram_svg_font_bridge_registration_tests.rs"]
mod registration_tests;

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

const X_HEIGHT_MEASUREMENT_FAILURE_SCRIPT: &str = r#"(() => {
    const measure = globalThis.__katanaMeasureSvgFontXHeight;
    const cases = [
        ['missing-family', () => measure('KRR Missing SVG Font Family', 16, 400, false)],
        ['empty-family', () => measure('', 16, 400, false)],
        ['omitted-family', () => measure()],
    ];
    return cases.map(([name, invoke]) => {
        try {
            invoke();
            return [name, 'resolved'].join(':');
        }
        catch (error) {
            return [name, error instanceof Error ? error.message : String(error)].join(':');
        }
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
        for initial_advance in [None, Some(0.0)] {
            assert_frozen_write_rejected(scope, initial_advance);
        }
    });
}

fn assert_frozen_write_rejected(scope: &mut v8::PinScope<'_, '_>, initial_advance: Option<f64>) {
    let object = v8::Object::new(scope);
    let key = v8::String::new(scope, "advance");
    assert!(key.is_some(), "key allocation");
    if let Some(key) = key {
        if let Some(value) = initial_advance {
            let value = v8::Number::new(scope, value);
            assert_eq!(object.set(scope, key.into(), value.into()), Some(true));
        }
        assert_eq!(
            object.set_integrity_level(scope, v8::IntegrityLevel::Frozen),
            Some(true)
        );
        assert_eq!(
            set_svg_metric_property(scope, object, "advance", 1.0),
            Err("SVG font metric result allocation failed")
        );
        match initial_advance {
            Some(value) => {
                assert_number_property(scope, object, "advance", value);
                assert_frozen_advance_descriptor(scope, object, key.into());
            }
            None => assert_property_presence(scope, object, "advance", false),
        }
    }
}

fn assert_frozen_advance_descriptor(
    scope: &mut v8::PinScope<'_, '_>,
    object: v8::Local<'_, v8::Object>,
    key: v8::Local<'_, v8::Name>,
) {
    let descriptor = object
        .get_own_property_descriptor(scope, key)
        .and_then(|value| value.to_object(scope));
    let descriptor_values = descriptor.and_then(|descriptor| {
        let value_key = v8::String::new(scope, "value")?;
        let writable_key = v8::String::new(scope, "writable")?;
        let configurable_key = v8::String::new(scope, "configurable")?;
        let value = descriptor.get(scope, value_key.into())?;
        let writable = descriptor.get(scope, writable_key.into())?;
        let configurable = descriptor.get(scope, configurable_key.into())?;
        Some((
            value.number_value(scope),
            writable.boolean_value(scope),
            configurable.boolean_value(scope),
        ))
    });
    assert_eq!(descriptor_values, Some((Some(0.0), false, false)));
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
fn metric_object_reports_rejected_optional_layout_ascent() {
    DiagramV8Runtime::ensure_initialized();
    DiagramV8Runtime::with_empty_try_catch(|scope| {
        let source = concat!(
            "new Proxy({}, { defineProperty(target, key, descriptor) { ",
            "if (key === 'layoutAscent') throw new Error('layoutAscent rejected'); ",
            "return Reflect.defineProperty(target, key, descriptor); } })",
        );
        let object = v8::String::new(scope, source).and_then(|source| {
            let name = v8::String::new(scope, "svg-font-rejected-layout-ascent")?;
            ScriptCompiler::compile_with_origin(scope, source, name)?
                .run(scope)?
                .to_object(scope)
        });
        assert!(
            object.is_some(),
            "selective property rejection fixture must run"
        );
        if let Some(object) = object {
            assert_eq!(
                set_svg_metric_properties(scope, object, metric_fixture(true)),
                Err("SVG font metric result allocation failed")
            );
            assert!(scope.has_caught());
            assert!(scope.exception().is_some_and(|error| {
                error.to_rust_string_lossy(scope).contains("layoutAscent")
            }));
        }
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

#[test]
fn real_v8_x_height_bridge_returns_bundled_font_metric() -> Result<(), String> {
    let script = r#"(() => {
        const value = globalThis.__katanaMeasureSvgFontXHeight('Noto Sans', 16, 400, false);
        return Number.isFinite(value) && value > 0;
    })()"#;
    let observed = DiagramV8Runtime::render(&[
        crate::markdown::diagram_js_runtime::DiagramRuntimeScript::borrowed(
            "bridge-valid-x-height",
            script,
        ),
    ])?;
    assert_eq!(observed, "true");
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn real_v8_x_height_bridge_leaves_metric_undefined_for_symbol_font() -> Result<(), String> {
    let script = r#"(() => {
        const value = globalThis.__katanaMeasureSvgFontXHeight('Zapf Dingbats', 16, 400, false);
        return value === undefined;
    })()"#;
    let observed = DiagramV8Runtime::render(&[
        crate::markdown::diagram_js_runtime::DiagramRuntimeScript::borrowed(
            "bridge-optional-symbol-x-height",
            script,
        ),
    ])?;
    assert_eq!(observed, "true");
    Ok(())
}

#[test]
fn real_v8_x_height_bridge_rejects_missing_family_measurements() -> Result<(), String> {
    let observed = DiagramV8Runtime::render(&[
        crate::markdown::diagram_js_runtime::DiagramRuntimeScript::borrowed(
            "bridge-x-height-measurement-failure-table",
            X_HEIGHT_MEASUREMENT_FAILURE_SCRIPT,
        ),
    ])?;
    assert_eq!(
        observed,
        concat!(
            "missing-family:SVG font x-height measurement failed,",
            "empty-family:SVG font x-height measurement failed,",
            "omitted-family:SVG font x-height measurement failed"
        )
    );
    Ok(())
}
