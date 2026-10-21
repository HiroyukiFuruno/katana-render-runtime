use super::test_support::Fixture;
use super::{RUST_INPUTS, mathjax_fixture};
use std::path::Path;

const MATHJAX_RUNTIME_PATH_RESOLVER_INDEX: usize = 3;
const DIAGRAM_JS_RUNTIME_INDEX: usize = 5;
const V8_SCRIPT_ORIGIN_HELPER_INDEX: usize = 6;
const LIB_MODULE_REGISTRATION_INDEX: usize = 7;
#[path = "runtime_v8_route_plantuml_tests.rs"]
mod plantuml_tests;

const RESOLVER_SHAPE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "if let Some(path) = runtime_path {\n            return Ok(path);\n        }",
        "match runtime_path {\n            Some(path) => return Ok(path),\n            None => {}\n        }",
        "runtime-path-resolver:early-return",
    ),
    (
        "if let Some(path) = runtime_path {\n            return Ok(path);\n        }",
        "if runtime_path.is_some() { return Ok(PathBuf::new()); }",
        "runtime-path-resolver:early-return",
    ),
    (
        "match kind {",
        "let _extra = (); match kind {",
        "runtime-path-resolver:statement-count",
    ),
    (
        "|path| {",
        "|path, other| {",
        "runtime-path-resolver:plantuml-closure",
    ),
    (
        "DiagramKind::Mermaid => MermaidBinaryOps::resolve_mermaid_js(),",
        "DiagramKind::Mermaid => DrawioRendererOps::resolve_drawio_js(),",
        "runtime-path-resolver:arm:DiagramKind::Mermaid",
    ),
    (
        "return Ok(path);",
        "return Ok(PathBuf::new());",
        "runtime-path-resolver:early-return",
    ),
    (
        "            DiagramKind::PlantUml => Ok(plantuml_cache_dir\n                .map_or_else(PlantUmlRendererOps::default_jar_path, |path| {\n                    PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)\n                })),\n",
        "",
        "runtime-path-resolver:dispatch-shape",
    ),
    (
        "            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),\n",
        "            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),\n            DiagramKind::Mermaid => MermaidBinaryOps::resolve_mermaid_js(),\n",
        "runtime-path-resolver:dispatch-shape",
    ),
    (
        "map_err(RenderError::RuntimeResolution)",
        "map_err()",
        "runtime-path-resolver:map-err",
    ),
    (
        "map_err(RenderError::RuntimeResolution)",
        "map_err(RenderError::RuntimeResolution, ForgedError::RuntimeResolution)",
        "runtime-path-resolver:map-err",
    ),
    (
        ".map_err(RenderError::RuntimeResolution)",
        "",
        "runtime-path-resolver:dispatch",
    ),
    (
        "map_err(RenderError::RuntimeResolution)",
        "map(RenderError::RuntimeResolution)",
        "runtime-path-resolver:map-err",
    ),
    (
        "DiagramKind::PlantUml => Ok(plantuml_cache_dir",
        "DiagramKind::PlantUml => Some(plantuml_cache_dir",
        "runtime-path-resolver:plantuml-arm",
    ),
    (
        "DiagramKind::PlantUml => Ok(plantuml_cache_dir",
        "DiagramKind::MathJax => Ok(plantuml_cache_dir",
        "runtime-path-resolver:arm:DiagramKind::PlantUml",
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

    plantuml_tests::reject_mutations(fixture, resolver, &original)
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

    reject_resolver_shape_mutations(fixture, resolver, &original)
}

fn reject_resolver_shape_mutations(
    fixture: &Fixture,
    resolver: &str,
    original: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(original)?;
    for (before, after, expected) in RESOLVER_SHAPE_MUTATIONS {
        assert_eq!(
            source.matches(*before).count(),
            1,
            "ambiguous mutation: {before}"
        );
        fixture.replace(resolver, before, after)?;
        fixture.reject(expected).map_err(|error| {
            format!("resolver mutation {before:?} -> {after:?} ({expected}): {error}")
        })?;
        fixture.restore_from(resolver, original)?;
    }
    reject_non_match_dispatch_receiver(fixture, resolver, original)?;
    Ok(())
}

fn reject_non_match_dispatch_receiver(
    fixture: &Fixture,
    resolver: &str,
    original: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    fixture.replace(resolver, "match kind {", "(match kind {")?;
    fixture.replace(
        resolver,
        "        }\n        .map_err(RenderError::RuntimeResolution)",
        "        })\n        .map_err(RenderError::RuntimeResolution)",
    )?;
    fixture.reject("runtime-path-resolver:dispatch")?;
    fixture.restore_from(resolver, original)?;
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
    for (before, after, expected) in [
        (
            "v8::String::new(scope, script_code)",
            "v8::String::new(scope, script.name)",
            "v8-compile-order:allocation",
        ),
        (
            "v8::String::new(scope, script.name)",
            "v8::String::new(scope, script_code)",
            "v8-compile-order:allocation",
        ),
        (
            "script.code.as_ref()",
            "script.name.as_ref()",
            "v8-compile-order:method:as_ref",
        ),
    ] {
        let isolated = mathjax_fixture()?;
        assert!(
            std::fs::read_to_string(isolated.root().join(v8_runtime))?.contains(before),
            "missing v8 allocation mutation: {before}"
        );
        isolated.replace(v8_runtime, before, after)?;
        isolated.reject(expected)?;
    }
    Ok(())
}

fn reject_v8_compile_mutations(v8_runtime: &str) -> Result<(), Box<dyn std::error::Error>> {
    for (before, after, expected) in [
        (
            "ScriptCompiler::compile_with_origin(scope, source, origin_name)",
            "ScriptCompiler::compile(scope, source)",
            "v8-compile-order:compile",
        ),
        (
            "required(v8::String::new(scope, script_code), SOURCE_ALLOC_ERR)",
            "maybe_required(v8::String::new(scope, script_code), SOURCE_ALLOC_ERR)",
            "v8-compile-order:required",
        ),
        (
            "let resolved = resolve_value(scope, value)?;",
            "let _extra = value;\n    let resolved = resolve_value(scope, value)?;",
            "v8-compile-order:statement-count",
        ),
    ] {
        let isolated = mathjax_fixture()?;
        assert!(
            std::fs::read_to_string(isolated.root().join(v8_runtime))?.contains(before),
            "missing v8 compile mutation: {before}"
        );
        isolated.replace(v8_runtime, before, after)?;
        isolated.reject(expected)?;
    }
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
