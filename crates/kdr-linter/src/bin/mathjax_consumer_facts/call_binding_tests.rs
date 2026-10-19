use super::*;

const REQUEST_JSON_MUTATIONS: &[(&str, &str, &str)] = &[
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

fn reject_mutations(
    fixture: &Fixture,
    relative: &str,
    mutations: &[(&str, &str, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    let original = repository_root()?.join(relative);
    for (before, after, diagnostic) in mutations {
        fixture.replace(relative, before, after)?;
        fixture.reject(diagnostic)?;
        fixture.restore_from(relative, &original)?;
    }
    Ok(())
}
