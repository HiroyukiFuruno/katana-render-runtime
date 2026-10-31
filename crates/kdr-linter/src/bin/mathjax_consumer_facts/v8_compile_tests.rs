use super::require_origin_constructor;

const ORIGIN_ERROR: &str = "v8-script-origin-helper:origin";
const COMPILE_ERROR: &str = "v8-script-origin-helper:compile";
const ORIGIN_ARGUMENTS: [&str; 11] = [
    "scope",
    "origin_name.into()",
    "0",
    "0",
    "false",
    "0",
    "Some(origin_name.into())",
    "false",
    "false",
    "false",
    "None",
];
const ORIGIN_ARGUMENT_VARIANTS: &[(usize, &str)] = &[
    (0, "other_scope"),
    (2, "1"),
    (3, "1"),
    (4, "true"),
    (5, "1"),
    (6, "None"),
    (7, "true"),
    (8, "true"),
    (9, "true"),
    (10, "Some(())"),
];
const INVALID_COMPILE_CALLS: &[&str] = &[
    "v8::Other::compile(scope, source, Some(&origin))",
    "v8::Script::compile(other_scope, source, Some(&origin))",
    "v8::Script::compile(scope, other_source, Some(&origin))",
    "v8::Script::compile(scope, source, Some(origin))",
    "v8::Script::compile(scope, source)",
    "v8::Script::compile(scope, source, Some(&origin), extra)",
];

#[test]
fn origin_constructor_rejects_zero_arguments_without_panicking() -> Result<(), String> {
    let expression = syn::parse_str::<syn::Expr>("v8::ScriptOrigin::new()")
        .map_err(|error| error.to_string())?;
    assert_eq!(
        require_origin_constructor(&expression).err().as_deref(),
        Some(ORIGIN_ERROR)
    );
    Ok(())
}

#[test]
fn origin_constructor_enforces_the_individual_v8_argument_contract() -> Result<(), String> {
    let valid = origin_constructor(&ORIGIN_ARGUMENTS)?;
    assert_eq!(require_origin_constructor(&valid), Ok(()));
    for (index, replacement) in ORIGIN_ARGUMENT_VARIANTS {
        let mut arguments = ORIGIN_ARGUMENTS.to_vec();
        arguments[*index] = replacement;
        let expression = origin_constructor(&arguments)?;
        assert_eq!(
            require_origin_constructor(&expression).err().as_deref(),
            Some(ORIGIN_ERROR)
        );
    }
    Ok(())
}

#[test]
fn origin_constructor_checks_short_and_long_argument_lists() -> Result<(), String> {
    let short = origin_constructor(&ORIGIN_ARGUMENTS[..ORIGIN_ARGUMENTS.len() - 1])?;
    let mut long_arguments = ORIGIN_ARGUMENTS.to_vec();
    long_arguments.push("extra");
    let long = origin_constructor(&long_arguments)?;
    for expression in [short, long] {
        assert_eq!(
            require_origin_constructor(&expression).err().as_deref(),
            Some(ORIGIN_ERROR)
        );
    }
    Ok(())
}

#[test]
fn script_compile_requires_the_origin_borrow_and_expected_arguments() -> Result<(), String> {
    for source in INVALID_COMPILE_CALLS {
        let expression = syn::parse_str::<syn::Expr>(source).map_err(|error| error.to_string())?;
        assert_eq!(
            super::require_origin_compile(&expression).err().as_deref(),
            Some(COMPILE_ERROR)
        );
    }
    Ok(())
}

#[test]
fn private_module_requires_an_out_of_line_declaration() -> Result<(), String> {
    let accepted = syn::parse_file("mod v8_script;").map_err(|error| error.to_string())?;
    assert_eq!(super::private_module(&accepted)?.ident, "v8_script");
    for source in ["struct Other;", "mod v8_script {}"] {
        let file = syn::parse_file(source).map_err(|error| error.to_string())?;
        assert_eq!(
            super::private_module(&file).err().as_deref(),
            Some("v8-script-origin-helper:module-registration")
        );
    }
    Ok(())
}

#[test]
fn compiler_import_requires_the_unaliased_v8_script_path() -> Result<(), String> {
    let accepted = syn::parse_file("use crate::v8_script::ScriptCompiler;")
        .map_err(|error| error.to_string())?;
    assert_eq!(super::require_compiler_import(&accepted), Ok(()));
    for source in [
        "",
        "use crate::other::ScriptCompiler;",
        "use crate::v8_script::ScriptCompiler as Compiler;",
    ] {
        let file = syn::parse_file(source).map_err(|error| error.to_string())?;
        assert_eq!(
            super::require_compiler_import(&file).err().as_deref(),
            Some("v8-script-origin-helper:diagram-import")
        );
    }
    Ok(())
}

#[test]
fn error_constants_must_include_each_expected_string_value() -> Result<(), String> {
    const EXPECTED: &[(&str, &str)] = &[("COMPILE_ERROR", "compile failed")];
    let valid = syn::parse_file("const COMPILE_ERROR: &str = \"compile failed\";")
        .map_err(|error| error.to_string())?;
    assert_eq!(super::require_error_constants(&valid, EXPECTED), Ok(()));
    for source in [
        "",
        "const COMPILE_ERROR: &str = \"other error\";",
        "const COMPILE_ERROR: usize = 1;",
    ] {
        let file = syn::parse_file(source).map_err(|error| error.to_string())?;
        assert_eq!(
            super::require_error_constants(&file, EXPECTED)
                .err()
                .as_deref(),
            Some("v8-compile-order:allocation-error")
        );
    }
    Ok(())
}

fn origin_constructor(arguments: &[&str]) -> Result<syn::Expr, String> {
    let arguments = arguments.join(", ");
    syn::parse_str(&format!("v8::ScriptOrigin::new({arguments})"))
        .map_err(|error| error.to_string())
}
