use super::{same_bytes_body, standard_path_binding};
use syn::Block;

const EXPECTED_BYTES: &str = "MathJaxRuntimeScripts::runtime_source().as_bytes()";
const READ_ARM: &str =
    "Ok(existing) => Ok(existing == MathJaxRuntimeScripts::runtime_source().as_bytes())";
const NOT_FOUND_ARM: &str =
    "Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false)";
const ERROR_ARM: &str = "Err(error) => Err(runtime_asset_error(error))";
const READ_SHAPE_ERROR: &str = "default-materializer:read-shape";
const READ_BYTES_ERROR: &str = "default-materializer:read-bytes-match";
const NOT_FOUND_ERROR: &str = "default-materializer:not-found-match";
const READ_ERROR: &str = "default-materializer:read-error-match";

#[test]
fn same_bytes_body_checks_match_shape_and_read_call() -> Result<(), syn::Error> {
    assert_body_case(
        "canonical readback",
        &match_body("std::fs::read(path)", &canonical_arms()),
        None,
    )?;
    assert_body_case(
        "non-match statement",
        "{ let marker = true; }",
        Some(READ_SHAPE_ERROR),
    )?;
    assert_body_case(
        "wrong read function",
        &match_body("other::read(path)", &canonical_arms()),
        Some(READ_SHAPE_ERROR),
    )?;
    assert_body_case(
        "wrong arm count",
        &match_body(
            "std::fs::read(path)",
            &format!("{READ_ARM}, {NOT_FOUND_ARM}"),
        ),
        Some(READ_SHAPE_ERROR),
    )?;
    Ok(())
}

#[test]
fn same_bytes_body_checks_success_arm_shapes() -> Result<(), syn::Error> {
    assert_success_arm_case(
        "non-tuple success pattern",
        &format!("existing => Ok(existing == {EXPECTED_BYTES})"),
    )?;
    assert_success_arm_case("success body without call", "Ok(existing) => true")?;
    assert_success_arm_case(
        "success call wrong arity",
        "Ok(existing) => Ok(existing, false)",
    )?;
    assert_success_arm_case("success value not comparison", "Ok(existing) => Ok(true)")?;
    assert_success_arm_case(
        "success comparison wrong binding",
        &format!("Ok(existing) => Ok(other == {EXPECTED_BYTES})"),
    )?;
    assert_success_arm_case(
        "success comparison wrong bytes method",
        "Ok(existing) => Ok(existing == other.as_bytes())",
    )?;
    Ok(())
}

#[test]
fn same_bytes_body_checks_error_arm_shapes() -> Result<(), syn::Error> {
    assert_not_found_arm_case(
        "not-found pattern is not tuple",
        "error if error.kind() == std::io::ErrorKind::NotFound => Ok(false)",
    )?;
    assert_not_found_arm_case(
        "not-found guard is not binary",
        "Err(error) if true => Ok(false)",
    )?;
    assert_not_found_arm_case(
        "not-found body is not a call",
        "Err(error) if error.kind() == std::io::ErrorKind::NotFound => false",
    )?;
    assert_error_arm_case(
        "error pattern is not tuple",
        "error => Err(runtime_asset_error(error))",
    )?;
    Ok(())
}

fn canonical_arms() -> String {
    format!("{READ_ARM}, {NOT_FOUND_ARM}, {ERROR_ARM}")
}

fn assert_success_arm_case(case: &str, arm: &str) -> Result<(), syn::Error> {
    let arms = format!("{arm}, {NOT_FOUND_ARM}, {ERROR_ARM}");
    assert_body_case(
        case,
        &match_body("std::fs::read(path)", &arms),
        Some(READ_BYTES_ERROR),
    )
}

fn assert_not_found_arm_case(case: &str, arm: &str) -> Result<(), syn::Error> {
    let arms = format!("{READ_ARM}, {arm}, {ERROR_ARM}");
    assert_body_case(
        case,
        &match_body("std::fs::read(path)", &arms),
        Some(NOT_FOUND_ERROR),
    )
}

fn assert_error_arm_case(case: &str, arm: &str) -> Result<(), syn::Error> {
    let arms = format!("{READ_ARM}, {NOT_FOUND_ARM}, {arm}");
    assert_body_case(
        case,
        &match_body("std::fs::read(path)", &arms),
        Some(READ_ERROR),
    )
}

fn assert_body_case(
    case: &str,
    source: &str,
    expected_error: Option<&str>,
) -> Result<(), syn::Error> {
    let block = syn::parse_str::<Block>(source)?;
    let result = same_bytes_body(&block);
    assert_eq!(result.err().as_deref(), expected_error, "{case}");
    Ok(())
}

fn match_body(read_call: &str, arms: &str) -> String {
    format!("{{ match {read_call} {{ {arms} }} }}")
}

#[test]
fn standard_path_import_rejects_unqualified_root_path_tree() -> Result<(), syn::Error> {
    let file = syn::parse_str::<syn::File>("use std::Path;")?;
    assert!(!standard_path_binding(&file));
    Ok(())
}
