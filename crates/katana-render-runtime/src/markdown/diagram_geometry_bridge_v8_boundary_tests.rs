use super::super::{TransformedBox, set_box_properties, set_number_property};
use crate::markdown::diagram_js_runtime::{
    DiagramRuntimeScript, DiagramTryCatchScope, DiagramV8Runtime,
};
use crate::v8_script::ScriptCompiler;

const REQUESTS: &str = r#"(() => {
    const transform = globalThis.__katanaTransformSvgBox;
    const cases = [
        ['negative-height', () => transform('', 0, 0, 1, -1)],
        ['nan-y', () => transform('', 0, NaN, 1, 1)],
        ['missing-height', () => transform('', 0, 0, 1)],
        ['infinite-x', () => transform('', Infinity, 0, 1, 1)],
        ['valid-coercion', () => transform('translate(3 4)', '-2', '-1', '2', '0')],
    ];
    return JSON.stringify(cases.map(([name, invoke]) => {
        try {
            return {name, value: invoke()};
        } catch (error) {
            return {name, error: error.message};
        }
    }));
})()"#;

const TEST_BOUNDS: TransformedBox = TransformedBox {
    x: 0.0,
    y: 0.0,
    width: 2.0,
    height: 1.0,
};

#[test]
fn real_v8_rejects_invalid_bounds_and_accepts_zero_height() -> Result<(), String> {
    let output = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "geometry-bridge-additional-boundaries",
        REQUESTS,
    )])?;
    let value: serde_json::Value =
        serde_json::from_str(&output).map_err(|error| error.to_string())?;
    assert_eq!(
        value,
        serde_json::json!([
            {"name": "negative-height", "error": "Invalid SVG child bounds"},
            {"name": "nan-y", "error": "Invalid SVG child bounds"},
            {"name": "missing-height", "error": "Invalid SVG child bounds"},
            {"name": "infinite-x", "error": "Invalid SVG child bounds"},
            {"name": "valid-coercion", "value": {"x": 1, "y": 3, "width": 2, "height": 0}}
        ])
    );
    Ok(())
}

#[test]
fn throwing_v8_setters_report_bounds_property_write_errors() {
    DiagramV8Runtime::ensure_initialized();
    DiagramV8Runtime::with_empty_try_catch(|scope| {
        let source = concat!(
            "new Proxy({}, {",
            "set(_target, property) { throw new Error(property + ' rejected'); },",
            "defineProperty(_target, property) { throw new Error(property + ' rejected'); }",
            "})",
        );
        let object = v8::String::new(scope, source).and_then(|source| {
            let name = v8::String::new(scope, "geometry-throwing-setters")?;
            let script = ScriptCompiler::compile_with_origin(scope, source, name)?;
            script.run(scope)?.to_object(scope)
        });
        assert!(
            object.is_some(),
            "throwing setter fixture must compile and run"
        );
        if let Some(object) = object {
            assert_throwing_setters(scope, object);
        }
    });
}

fn assert_throwing_setters(
    scope: &mut DiagramTryCatchScope<'_, '_, '_, '_>,
    object: v8::Local<'_, v8::Object>,
) {
    let expected = Err("SVG transformed bounds property write failed".to_string());
    assert_eq!(set_number_property(scope, object, "width", 3.0), expected);
    assert!(scope.has_caught());
    assert!(
        scope
            .exception()
            .is_some_and(|error| error.to_rust_string_lossy(scope).contains("width rejected"))
    );
    scope.reset();
    assert_eq!(set_box_properties(scope, object, TEST_BOUNDS), expected);
    assert!(scope.has_caught());
    assert!(
        scope
            .exception()
            .is_some_and(|error| error.to_rust_string_lossy(scope).contains("x rejected"))
    );
    scope.reset();
}

#[test]
fn v8_box_writer_reports_rejected_properties_after_prior_fields() {
    DiagramV8Runtime::ensure_initialized();
    for field in ["y", "width", "height"] {
        DiagramV8Runtime::with_empty_try_catch(|scope| assert_box_field_write_fails(scope, field));
    }
}

fn assert_box_field_write_fails(scope: &mut DiagramTryCatchScope<'_, '_, '_, '_>, field: &str) {
    let source = format!(
        "new Proxy({{}}, {{defineProperty(target, key, descriptor) {{ \
         if (key === '{field}') throw new Error(key + ' rejected'); \
         return Reflect.defineProperty(target, key, descriptor); }}}})"
    );
    let object = v8::String::new(scope, &source).and_then(|source| {
        let name = v8::String::new(scope, "geometry-selective-property-rejection")?;
        ScriptCompiler::compile_with_origin(scope, source, name)?
            .run(scope)?
            .to_object(scope)
    });
    assert!(object.is_some(), "selective setter fixture must run");
    if let Some(object) = object {
        assert_eq!(
            set_box_properties(scope, object, TEST_BOUNDS)
                .err()
                .as_deref(),
            Some("SVG transformed bounds property write failed"),
        );
        assert!(scope.exception().is_some_and(|error| {
            error
                .to_rust_string_lossy(scope)
                .contains(&format!("{field} rejected"))
        }));
    }
}

#[test]
fn geometry_bridge_rejects_throwing_registration_setter() {
    DiagramV8Runtime::ensure_initialized();
    for (property, expected) in [
        (
            "__katanaTransformSvgBox",
            "SVG transform bridge registration failed",
        ),
        (
            "__katanaSvgPathLength",
            "SVG path length bridge registration failed",
        ),
    ] {
        assert_registration_setter_rejected(property, expected);
    }
    #[cfg(target_os = "macos")]
    assert_registration_setter_rejected(
        "__katanaSvgPathBounds",
        "SVG path bounds bridge registration failed",
    );
}

fn assert_registration_setter_rejected(property: &str, expected: &str) {
    DiagramV8Runtime::with_empty_try_catch(|scope| {
        let source = format!(
            "Object.defineProperty(globalThis, '{property}', \
             {{set() {{ throw new Error('blocked'); }}, configurable: true}}); true;"
        );
        let configured = v8::String::new(scope, &source).and_then(|source| {
            let name = v8::String::new(scope, "geometry-registration-rejection")?;
            let script = ScriptCompiler::compile_with_origin(scope, source, name)?;
            script.run(scope)
        });
        assert!(configured.is_some());
        assert_eq!(super::super::install(scope), Err(expected.to_owned()));
        assert!(scope.has_caught());
        assert!(
            scope
                .exception()
                .is_some_and(|error| error.to_rust_string_lossy(scope).contains("blocked"))
        );
    });
}
