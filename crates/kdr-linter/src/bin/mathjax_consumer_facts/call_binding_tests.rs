use super::*;

const RENDER_ROUTE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "let output = DiagramV8Runtime::render(&scripts)?;\n        match parse_response(&output)? {",
        "let output = DiagramV8Runtime::render(&scripts)?;\n        let _route_contract = &output;\n        match parse_response(&output)? {",
        "read-render-route:statement-count",
    ),
    (
        "MathJaxRuntimeResponse::Error { message } => Err(message),",
        "MathJaxRuntimeResponse::Error { message: other } => Err(other),",
        "read-render-route:response-binding",
    ),
    (
        "MathJaxRuntimeResponse::Svg { svg } => Ok(svg),",
        "MathJaxRuntimeResponse::Svg(svg) => Ok(svg),",
        "read-render-route:response-pattern",
    ),
    (
        "MathJaxRuntimeResponse::Svg { svg } => Ok(svg),",
        "MathJaxRuntimeResponse::Error { svg } => Ok(svg),",
        "read-render-route:response-pattern",
    ),
    (
        "MathJaxRuntimeResponse::Error { message } => Err(message),",
        "MathJaxRuntimeResponse::Error { message } => Err(message),\n            _ => Err(\"unexpected response\".to_string()),",
        "read-render-route:response-arms",
    ),
    (
        "let scripts = MathJaxRuntimeScripts::build(mathjax_source, &request_json);",
        "let scripts = MathJaxRuntimeScripts::build(&request_json, mathjax_source);",
        "read-render-route:argument:mathjax_source",
    ),
    (
        "let output = DiagramV8Runtime::render(&scripts)?;",
        "let output = DiagramV8Runtime::render(&request_json)?;",
        "read-render-route:argument:scripts",
    ),
    (
        "match parse_response(&output)? {\n            MathJaxRuntimeResponse::Svg { svg } => Ok(svg),\n            MathJaxRuntimeResponse::Error { message } => Err(message),\n        }",
        "if output.is_empty() { Ok(String::new()) } else { Err(String::new()) }",
        "read-render-route:response-match",
    ),
];

const REQUEST_JSON_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "request.to_json_value().to_string()",
        "request",
        "read-render-route:request-json",
    ),
    (
        "request.to_json_value().to_string()",
        "(request.to_json_value()).to_string()",
        "read-render-route:request-json",
    ),
    (
        "request.to_json_value().to_string()",
        "other.to_json_value().to_string()",
        "read-render-route:request-json",
    ),
    (
        "request.to_json_value().to_string()",
        "request.to_json_value(other).to_string()",
        "read-render-route:request-json",
    ),
    (
        "request.to_json_value().to_string()",
        "request.to_json_value().to_string(other)",
        "read-render-route:request-json",
    ),
];

const SOURCE_READER_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "Err(error) => Err(format!(",
        "Ok(error) => Err(format!(",
        "read-render-route:source-reader-other-pattern",
    ),
    (
        "error.kind() == std::io::ErrorKind::NotFound",
        "error",
        "read-render-route:source-reader-not-found-pattern",
    ),
    (
        "Err(format!(",
        "Ok(format!(",
        "read-render-route:source-reader-not-found-error",
    ),
    (
        "Err(format!(",
        "Err(concat!(",
        "read-render-route:source-reader-not-found-error",
    ),
    (
        "fn runtime_source_from(mathjax_js: &Path) -> Result<String, String> {\n    match",
        "fn runtime_source_from(mathjax_js: &Path) -> Result<String, String> {\n    let _ = mathjax_js;\n    match",
        "read-render-route:source-reader-body",
    ),
    (
        "std::fs::read_to_string(mathjax_js)",
        "std::fs::read_to_string(other_path)",
        "read-render-route:source-reader-call",
    ),
    (
        "std::fs::read_to_string(mathjax_js)",
        "source_result",
        "read-render-route:source-reader-call",
    ),
    (
        "std::io::ErrorKind::NotFound",
        "std::io::ErrorKind::PermissionDenied",
        "read-render-route:source-reader-not-found-pattern",
    ),
    (
        "error.kind() == std::io::ErrorKind::NotFound",
        "error.kind() != std::io::ErrorKind::NotFound",
        "read-render-route:source-reader-not-found-pattern",
    ),
    (
        "Ok(source) => Ok(source),",
        "Ok(_) => Err(\"runtime source rejected\".to_string()),",
        "read-render-route:source-reader-success-pattern",
    ),
    (
        "MathJax runtime asset is not installed: {}",
        "MathJax runtime asset is unavailable: {}",
        "read-render-route:source-reader-not-found-error",
    ),
    (
        "MathJax runtime asset could not be read: {}: {error}",
        "MathJax runtime asset read failed: {}: {error}",
        "read-render-route:source-reader-other-error",
    ),
];

const SOURCE_READER_SUCCESS_ARM: &str = "        Ok(source) => Ok(source),\n";
const SOURCE_READER_NOT_FOUND_ARM: &str = concat!(
    "        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(format!(\n",
    "            \"MathJax runtime asset is not installed: {}\",\n",
    "            mathjax_js.display()\n",
    "        )),\n",
);
const SOURCE_READER_OTHER_ARM: &str = concat!(
    "        Err(error) => Err(format!(\n",
    "            \"MathJax runtime asset could not be read: {}: {error}\",\n",
    "            mathjax_js.display()\n",
    "        )),\n",
);
const SOURCE_READER_PERMISSION_ARM: &str = concat!(
    "        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => ",
    "Err(\"permission denied\".to_string()),\n",
);

#[test]
fn rejects_request_json_from_wrong_receiver_or_call_arguments()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    reject_mutations(
        &fixture,
        RUST_INPUTS[MATHJAX_JS_RUNTIME_INDEX],
        REQUEST_JSON_MUTATIONS,
    )
}

#[test]
fn rejects_render_route_statement_and_response_shape_changes()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    reject_mutations(
        &fixture,
        RUST_INPUTS[MATHJAX_JS_RUNTIME_INDEX],
        RENDER_ROUTE_MUTATIONS,
    )
}

#[test]
fn rejects_runtime_source_reader_body_and_error_route_changes()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    reject_mutations(
        &fixture,
        RUST_INPUTS[MATHJAX_JS_RUNTIME_INDEX],
        SOURCE_READER_MUTATIONS,
    )
}

#[test]
fn rejects_source_reader_nonmatch_and_noncall_ast_shapes() -> Result<(), Box<dyn std::error::Error>>
{
    for (source, expected) in [
        ("fn other() {}", "missing-function:runtime_source_from"),
        (
            "fn runtime_source_from() { Ok(()) }",
            "read-render-route:source-reader-match",
        ),
        (
            "fn runtime_source_from() { match source_result { _ => Ok(()) } }",
            "read-render-route:source-reader-call",
        ),
    ] {
        let file = syn::parse_file(source)?;
        let rejection = super::super::call_binding::require_runtime_source_reader(&file).err();
        assert_eq!(rejection.as_deref(), Some(expected));
    }
    Ok(())
}

#[test]
fn rejects_runtime_source_reader_arm_counts_with_the_call_diagnostic()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    reject_runtime_source_reader_arm_counts(&fixture)
}

fn reject_runtime_source_reader_arm_counts(
    fixture: &Fixture,
) -> Result<(), Box<dyn std::error::Error>> {
    let relative = RUST_INPUTS[MATHJAX_JS_RUNTIME_INDEX];
    let original = format!(
        "{SOURCE_READER_SUCCESS_ARM}{SOURCE_READER_NOT_FOUND_ARM}{SOURCE_READER_OTHER_ARM}"
    );
    let permission_arm_count_variant = format!(
        "{SOURCE_READER_SUCCESS_ARM}{SOURCE_READER_NOT_FOUND_ARM}{SOURCE_READER_PERMISSION_ARM}{SOURCE_READER_OTHER_ARM}"
    );
    let variants = [
        String::new(),
        SOURCE_READER_SUCCESS_ARM.to_string(),
        format!("{SOURCE_READER_SUCCESS_ARM}{SOURCE_READER_OTHER_ARM}"),
        permission_arm_count_variant,
    ];
    let original_file = repository_root()?.join(relative);
    for replacement in variants {
        fixture.replace(relative, &original, &replacement)?;
        fixture.reject("read-render-route:source-reader-call")?;
        fixture.restore_from(relative, &original_file)?;
    }
    Ok(())
}

pub(super) fn reject_mutations(
    fixture: &Fixture,
    relative: &str,
    mutations: &[(&str, &str, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    let original = repository_root()?.join(relative);
    for (before, after, diagnostic) in mutations {
        fixture.replace(relative, before, after)?;
        fixture
            .reject(diagnostic)
            .map_err(|error| format!("mutation {before:?} => {after:?}: {error}"))?;
        fixture.restore_from(relative, &original)?;
    }
    Ok(())
}
