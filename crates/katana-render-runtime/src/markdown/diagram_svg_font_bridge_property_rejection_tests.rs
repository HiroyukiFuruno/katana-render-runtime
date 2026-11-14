use super::{DiagramV8Runtime, metric_fixture, set_svg_metric_properties};
use crate::v8_script::ScriptCompiler;

#[rstest::rstest]
#[case::ink_bounds("inkTop")]
#[case::layout_ascent("layoutAscent")]
fn metric_object_reports_rejected_property(#[case] rejected_property: &str) {
    DiagramV8Runtime::ensure_initialized();
    DiagramV8Runtime::with_empty_try_catch(|scope| {
        let object = rejecting_metric_object(scope, rejected_property);
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
                error
                    .to_rust_string_lossy(scope)
                    .contains(rejected_property)
            }));
        }
    });
}

fn rejecting_metric_object<'scope>(
    scope: &mut v8::PinScope<'scope, '_>,
    rejected_property: &str,
) -> Option<v8::Local<'scope, v8::Object>> {
    let source = format!(
        "new Proxy({{}}, {{ defineProperty(target, key, descriptor) {{ \
        if (key === '{rejected_property}') throw new Error(key + ' rejected'); \
        return Reflect.defineProperty(target, key, descriptor); }} }})",
    );
    let source = v8::String::new(scope, &source)?;
    let name = v8::String::new(scope, "svg-font-rejected-property")?;
    ScriptCompiler::compile_with_origin(scope, source, name)?
        .run(scope)?
        .to_object(scope)
}
