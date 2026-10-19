use super::syntax;
use syn::parse::Parser;

fn expr(source: &str) -> Result<syn::Expr, syn::Error> {
    syn::parse_str(source)
}

fn assert_cases(
    cases: &[(&str, bool)],
    predicate: impl Fn(&syn::Expr) -> bool,
) -> Result<(), Box<dyn std::error::Error>> {
    for (source, expected) in cases {
        assert_eq!(
            predicate(&expr(source)?),
            *expected,
            "unexpected result: {source}"
        );
    }
    Ok(())
}

#[test]
fn mathjax_syntax_return_predicates_reject_unverified_ast_shapes()
-> Result<(), Box<dyn std::error::Error>> {
    for (source, expected) in [("return Ok(path);", true), ("return Err(path);", false)] {
        let statement = syn::parse_str::<syn::Stmt>(source)?;
        assert_eq!(
            syntax::return_call(Some(&statement), "Ok", &["path"]),
            expected
        );
    }
    assert_cases(
        &[
            ("{ return Ok(path); }", true),
            ("{ let path = value; return Ok(path); }", false),
            ("return Ok(path)", true),
            ("return Wrong(path)", false),
            ("return", false),
            ("path", false),
        ],
        |value| syntax::return_expr_call(value, "Ok", &["path"]),
    )?;
    assert_cases(
        &[
            ("return Err(\"missing\".to_string())", true),
            ("return Err(\"other\".to_string())", false),
            ("{ let message = \"missing\"; return Err(message); }", false),
        ],
        |value| syntax::return_error(value, "missing"),
    )
}

#[test]
fn mathjax_syntax_return_call_rejects_absent_or_non_return_statements()
-> Result<(), Box<dyn std::error::Error>> {
    let statement = syn::parse_str::<syn::Stmt>("let path = value;")?;
    assert!(!syntax::return_call(None, "Ok", &["path"]));
    assert!(!syntax::return_call(Some(&statement), "Ok", &["path"]));
    Ok(())
}

#[test]
fn mathjax_syntax_some_binding_requires_single_named_binding()
-> Result<(), Box<dyn std::error::Error>> {
    for (source, binding, expected) in [
        ("Some(path)", "path", true),
        ("Some(other)", "path", false),
        ("Some(ref path)", "path", true),
        ("Some(path, other)", "path", false),
        ("None", "path", false),
        ("Other(path)", "path", false),
    ] {
        let pattern = syn::Pat::parse_single.parse_str(source)?;
        assert_eq!(
            syntax::is_some_binding(&pattern, binding),
            expected,
            "unexpected result: {source}"
        );
    }
    Ok(())
}

#[test]
fn mathjax_syntax_error_predicate_rejects_malformed_error_shapes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_cases(
        &[
            ("{ let message = value; }", false),
            ("{ value; }", false),
            ("return", false),
            ("return value", false),
            ("return Ok(message)", false),
            ("return Err(a, b)", false),
            ("return Err(message)", false),
            ("return Err(\"missing\".to_string(extra))", false),
            ("return Err(String::from(\"missing\"))", false),
        ],
        |value| syntax::return_error(value, "missing"),
    )
}

#[test]
fn mathjax_syntax_path_predicates_reject_unverified_ast_shapes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_cases(
        &[
            ("Ok(Some(PathBuf::from(path)))", true),
            ("Ok(Some(PathBuf::from(other)))", false),
            ("Ok(None)", false),
            ("path", false),
            ("Ok(path, other)", false),
            ("Ok(Other(path))", false),
            ("Ok(Some(path, other))", false),
            ("Ok(Some(path))", false),
            ("Ok(Some(PathBuf::other(path)))", false),
            ("Ok(Some(PathBuf::from(path, other)))", false),
        ],
        syntax::ok_some_path,
    )?;
    let block = syn::parse_str::<syn::Block>("{ let path = source; path }")?;
    assert!(syntax::local_or_tail(&block, 1).is_ok());
    assert!(syntax::local_or_tail(&block, 0).is_err());
    assert_cases(
        &[
            ("resolve(path)", true),
            ("resolve(other)", false),
            ("path", false),
        ],
        |value| syntax::call_expr_path(value, "resolve", &["path"]),
    )
}

#[test]
fn mathjax_syntax_success_render_rejects_unverified_ast_shapes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_cases(&success_render_cases(), syntax::require_success_render)
}

fn success_render_cases() -> Vec<(&'static str, bool)> {
    vec![
        (
            "RenderOutputFactory::from_diagram_result(input, crate::markdown::DiagramResult::Ok(svg), runtime)",
            true,
        ),
        (
            "RenderOutputFactory::from_diagram_result(input, crate::markdown::DiagramResult::Err(error), runtime)",
            false,
        ),
        (
            "RenderOutputFactory::from_diagram_result(input, crate::markdown::DiagramResult::Ok(svg), other_runtime)",
            false,
        ),
        (
            "RenderOutputFactory::from_diagram_result(input, svg, runtime)",
            false,
        ),
        (
            "RenderOutputFactory::from_diagram_result(input, crate::markdown::DiagramResult::Ok(svg))",
            false,
        ),
        (
            "Other(input, crate::markdown::DiagramResult::Ok(svg), runtime)",
            false,
        ),
        ("result", false),
    ]
}

#[test]
fn mathjax_syntax_warning_render_rejects_unverified_ast_shapes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_cases(
        &[
            (
                "Ok(RenderOutputFactory::from_raw_string(input, input.source.clone(), warning, runtime))",
                true,
            ),
            (
                "Ok(RenderOutputFactory::from_raw_string(input, input.source.clone(), warning, other_runtime))",
                false,
            ),
            (
                "Err(RenderOutputFactory::from_raw_string(input, input.source.clone(), warning, runtime))",
                false,
            ),
            (
                "Ok(RenderOutputFactory::from_raw_string(input, input.source.clone(), warning))",
                false,
            ),
            (
                "Ok(Other(input, input.source.clone(), warning, runtime))",
                false,
            ),
            ("Ok(input)", false),
            ("value", false),
        ],
        syntax::require_warning_render,
    )
}
