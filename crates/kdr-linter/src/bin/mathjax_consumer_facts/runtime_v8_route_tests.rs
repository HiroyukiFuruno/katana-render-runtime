use super::test_support::Fixture;
use super::{RUST_INPUTS, mathjax_fixture};
use std::path::Path;

const MATHJAX_RUNTIME_PATH_RESOLVER_INDEX: usize = 3;
const DIAGRAM_JS_RUNTIME_INDEX: usize = 5;
const V8_SCRIPT_ORIGIN_HELPER_INDEX: usize = 6;
const LIB_MODULE_REGISTRATION_INDEX: usize = 7;
const PLANTUML_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "PlantUmlRendererOps::default_jar_path",
        "FakePlantUmlRendererOps::default_jar_path",
        "runtime-path-resolver:plantuml-default",
    ),
    (
        ".map_or_else(PlantUmlRendererOps::default_jar_path",
        ".map_or(PlantUmlRendererOps::default_jar_path",
        "runtime-path-resolver:plantuml-map-or-else",
    ),
    (
        "PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)",
        "PlantUmlRendererOps::default_jar_path_for_cache_dir(path)",
        "runtime-path-resolver:plantuml-cache",
    ),
    (
        "PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)",
        "PlantUmlRendererOps::default_jar_path_for_cache_dir(&path.join(\"nested\"))",
        "runtime-path-resolver:plantuml-cache",
    ),
];

pub(super) fn reject_runtime_resolver_changes(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    reject_resolver_arm_and_dispatch(fixture, repository)?;
    reject_resolver_error_and_early_return(fixture, repository)
}

fn reject_resolver_arm_and_dispatch(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let resolver = RUST_INPUTS[MATHJAX_RUNTIME_PATH_RESOLVER_INDEX];
    let original = repository.join(resolver);
    fixture.replace(
        resolver,
        "DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),",
        "DiagramKind::MathJax => FakeMathJaxOps::resolve_mathjax_js(),",
    )?;
    fixture.reject("runtime-path-resolver:arm:DiagramKind::MathJax")?;
    fixture.restore_from(resolver, &original)?;

    fixture.replace(resolver, "match kind {", "match runtime_path {")?;
    fixture.reject("runtime-path-resolver:dispatch-shape")?;
    fixture.restore_from(resolver, &original)?;

    reject_plantuml_mutations(fixture, resolver, &original)
}

fn reject_plantuml_mutations(
    fixture: &Fixture,
    resolver: &str,
    original: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    for (before, after, expected) in PLANTUML_MUTATIONS {
        fixture.replace(resolver, before, after)?;
        fixture.reject(expected)?;
        fixture.restore_from(resolver, original)?;
    }
    Ok(())
}

fn reject_resolver_error_and_early_return(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let resolver = RUST_INPUTS[MATHJAX_RUNTIME_PATH_RESOLVER_INDEX];
    let original = repository.join(resolver);
    fixture.replace(
        resolver,
        "map_err(RenderError::RuntimeResolution)",
        "map_err(ForgedError::RuntimeResolution)",
    )?;
    fixture.reject("runtime-path-resolver:map-err")?;
    fixture.restore_from(resolver, &original)?;

    fixture.replace(
        resolver,
        "if let Some(path) = runtime_path {",
        "if let Some(path) = None {",
    )?;
    fixture.reject("runtime-path-resolver:early-return")?;
    fixture.restore_from(resolver, &original)?;
    Ok(())
}

pub(super) fn reject_v8_and_registration_changes(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    reject_v8_support_mutations(fixture, repository)?;
    reject_v8_runtime_mutations()
}

fn reject_v8_support_mutations(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let v8_helper = RUST_INPUTS[V8_SCRIPT_ORIGIN_HELPER_INDEX];
    let original = repository.join(v8_helper);
    fixture.replace(v8_helper, "origin_name.into(),", "source.into(),")?;
    fixture.reject("v8-script-origin-helper:origin")?;
    fixture.restore_from(v8_helper, &original)?;

    let registration = RUST_INPUTS[LIB_MODULE_REGISTRATION_INDEX];
    let original = repository.join(registration);
    fixture.replace(registration, "mod v8_script;", "pub mod v8_script;")?;
    fixture.reject("v8-script-origin-helper:module-registration")?;
    fixture.restore_from(registration, &original)?;

    Ok(())
}

fn reject_v8_runtime_mutations() -> Result<(), Box<dyn std::error::Error>> {
    let v8_runtime = RUST_INPUTS[DIAGRAM_JS_RUNTIME_INDEX];
    reject_v8_allocation_mutations(v8_runtime)?;
    reject_v8_compile_mutations(v8_runtime)?;
    reject_v8_result_mutations(v8_runtime)
}

fn reject_v8_allocation_mutations(v8_runtime: &str) -> Result<(), Box<dyn std::error::Error>> {
    for (before, after) in [
        (
            "v8::String::new(scope, script_code)",
            "v8::String::new(scope, script.name)",
        ),
        (
            "v8::String::new(scope, script.name)",
            "v8::String::new(scope, script_code)",
        ),
    ] {
        let isolated = mathjax_fixture()?;
        assert!(
            std::fs::read_to_string(isolated.root().join(v8_runtime))?.contains(before),
            "missing v8 allocation mutation: {before}"
        );
        isolated.replace(v8_runtime, before, after)?;
        isolated.reject("v8-compile-order:allocation")?;
    }
    Ok(())
}

fn reject_v8_compile_mutations(v8_runtime: &str) -> Result<(), Box<dyn std::error::Error>> {
    let before = "ScriptCompiler::compile_with_origin(scope, source, origin_name)";
    let after = "ScriptCompiler::compile(scope, source)";
    let isolated = mathjax_fixture()?;
    assert!(
        std::fs::read_to_string(isolated.root().join(v8_runtime))?.contains(before),
        "missing v8 mutation: {before}"
    );
    isolated.replace(v8_runtime, before, after)?;
    isolated.reject("v8-compile-order:compile")?;
    Ok(())
}

fn reject_v8_result_mutations(v8_runtime: &str) -> Result<(), Box<dyn std::error::Error>> {
    for (before, after, expected) in [
        (
            "script.run(scope)",
            "script.run(other_scope)",
            "v8-compile-order:run",
        ),
        (
            "exception_message(scope)",
            "exception_message(other_scope)",
            "v8-compile-order:compile",
        ),
        (
            "resolve_value(scope, value)",
            "resolve_value(other_scope, value)",
            "resolve",
        ),
    ] {
        let isolated = mathjax_fixture()?;
        assert!(
            std::fs::read_to_string(isolated.root().join(v8_runtime))?.contains(before),
            "missing v8 mutation: {before}"
        );
        isolated.replace(v8_runtime, before, after)?;
        isolated.reject(expected)?;
    }
    Ok(())
}
