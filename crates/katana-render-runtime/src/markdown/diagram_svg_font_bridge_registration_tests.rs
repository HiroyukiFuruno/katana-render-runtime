use super::DiagramV8Runtime;
use crate::markdown::diagram_js_runtime::DiagramTryCatchScope;
use crate::v8_script::ScriptCompiler;

const REGISTRATIONS: &[(&str, &str)] = &[
    (
        "__katanaMeasureText",
        "font measure bridge registration failed",
    ),
    (
        "__katanaMeasureCanvasText",
        "canvas font measure bridge registration failed",
    ),
    (
        "__katanaMeasureSvgTextMetrics",
        "SVG font metric bridge registration failed",
    ),
    (
        "__katanaMeasureSvgFontXHeight",
        "SVG font x-height bridge registration failed",
    ),
];
const READONLY_SENTINEL: i32 = 7;
const PROBE_VALUE: i32 = 8;

#[test]
fn font_bridge_reports_throwing_registration_setter() {
    DiagramV8Runtime::ensure_initialized();
    let fragment = (
        "__katanaMeasureSvgFragmentLayout",
        "SVG fragment bridge registration failed",
    );
    for &(property, expected) in REGISTRATIONS.iter().chain(std::iter::once(&fragment)) {
        assert_accessor_registration_rejected(
            property,
            expected,
            "set() { throw new Error('blocked'); }",
            "blocked",
        );
    }
}

#[test]
fn font_bridge_reports_throwing_registration_getter() {
    DiagramV8Runtime::ensure_initialized();
    for &(property, expected) in REGISTRATIONS {
        assert_accessor_registration_rejected(
            property,
            expected,
            "set(_) {}, get() { throw new Error('blocked getter'); }",
            "blocked getter",
        );
    }
}

#[test]
fn font_bridge_rejects_a_setter_that_accepts_but_discards_the_callback() {
    DiagramV8Runtime::ensure_initialized();
    DiagramV8Runtime::with_empty_try_catch(|scope| {
        let (property, expected) = REGISTRATIONS[0];
        let source = format!(
            "Object.defineProperty(globalThis, '{property}', {{\
             set(value) {{ globalThis.callbackKind = typeof value; }}, \
             get() {{ return 'unchanged'; }}, configurable: true}});"
        );
        assert!(configure(scope, &source));
        assert_eq!(
            super::super::super::diagram_font_bridge::install(scope),
            Err(expected.to_owned())
        );
        assert!(!scope.has_caught());
        assert!(configure(
            scope,
            &format!(
                "if (globalThis.callbackKind !== 'function' || globalThis.{property} !== 'unchanged') \
                 throw new Error('setter did not discard the callback');"
            )
        ));
    });
}

fn assert_accessor_registration_rejected(
    property: &str,
    expected: &str,
    descriptor: &str,
    message: &str,
) {
    DiagramV8Runtime::with_empty_try_catch(|scope| {
        let source = format!(
            "Object.defineProperty(globalThis, '{property}', \
             {{{descriptor}, configurable: true}}); true;"
        );
        assert!(configure(scope, &source));
        assert_eq!(
            super::super::super::diagram_font_bridge::install(scope),
            Err(expected.to_owned())
        );
        assert!(scope.has_caught());
        assert!(
            scope
                .exception()
                .is_some_and(|error| error.to_rust_string_lossy(scope).contains(message))
        );
    });
}

#[test]
fn font_bridge_rejects_readonly_registration_property() {
    DiagramV8Runtime::ensure_initialized();
    for &(property, expected) in REGISTRATIONS {
        assert_readonly_registration_rejected(property, expected);
    }
}

fn assert_readonly_registration_rejected(property: &str, expected: &str) {
    DiagramV8Runtime::with_empty_try_catch(|scope| {
        assert!(configure_readonly(scope, property));
        let context = scope.get_current_context();
        let global = context.global(scope);
        let key = v8::String::new(scope, property);
        assert!(key.is_some());
        if let Some(key) = key {
            let probe_value = v8::Number::new(scope, f64::from(PROBE_VALUE));
            let set_result = global.set(scope, key.into(), probe_value.into());
            scope.reset();
            let result = super::super::super::diagram_font_bridge::install(scope);
            scope.reset();
            let actual = global
                .get(scope, key.into())
                .and_then(|value| value.number_value(scope));
            assert_eq!(actual, Some(f64::from(READONLY_SENTINEL)));
            assert_eq!(
                result,
                Err(expected.to_owned()),
                "V8 set returned {set_result:?} but property remained {actual:?}"
            );
        }
    });
}

fn configure_readonly(scope: &mut DiagramTryCatchScope<'_, '_, '_, '_>, property: &str) -> bool {
    let source = format!(
        "Object.defineProperty(globalThis, '{property}', \
         {{value: {READONLY_SENTINEL}, writable: false, configurable: true}}); true;"
    );
    configure(scope, &source)
}

fn configure(scope: &mut DiagramTryCatchScope<'_, '_, '_, '_>, source: &str) -> bool {
    v8::String::new(scope, source)
        .and_then(|source| {
            let name = v8::String::new(scope, "font-readonly-registration")?;
            ScriptCompiler::compile_with_origin(scope, source, name)?.run(scope)
        })
        .is_some()
}
