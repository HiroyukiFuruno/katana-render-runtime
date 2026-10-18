use super::syntax;

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
fn mathjax_syntax_path_predicates_reject_unverified_ast_shapes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_cases(
        &[
            ("Ok(Some(PathBuf::from(path)))", true),
            ("Ok(Some(PathBuf::from(other)))", false),
            ("Ok(None)", false),
        ],
        syntax::ok_some_path,
    )?;
    let block = syn::parse_str::<syn::Block>("{ let path = source; path }")?;
    assert!(syntax::local_or_tail(&block, 1).is_ok());
    assert!(syntax::local_or_tail(&block, 0).is_err());
    assert_cases(
        &[("resolve(path)", true), ("resolve(other)", false)],
        |value| syntax::call_expr_path(value, "resolve", &["path"]),
    )
}

#[test]
fn mathjax_syntax_success_render_rejects_unverified_ast_shapes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_cases(
        &[
            (
                "RenderOutputFactory::from_diagram_result(input, crate::markdown::DiagramResult::Ok(svg), runtime)",
                true,
            ),
            (
                "RenderOutputFactory::from_diagram_result(input, crate::markdown::DiagramResult::Err(error), runtime)",
                false,
            ),
        ],
        syntax::require_success_render,
    )
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
        ],
        syntax::require_warning_render,
    )
}
