const ORIGIN_INITIALIZER: &str = concat!(
    "        let origin = v8::ScriptOrigin::new(\n",
    "            scope,\n            origin_name.into(),\n            0,\n            0,\n",
    "            false,\n            0,\n            Some(origin_name.into()),\n            false,\n",
    "            false,\n            false,\n            None,\n        );\n",
);
const ORIGIN_COMPILE: &str = "        v8::Script::compile(scope, source, Some(&origin))";
const ORIGIN_HELPER_BODY: &str = concat!(
    "        let origin = v8::ScriptOrigin::new(\n",
    "            scope,\n            origin_name.into(),\n            0,\n            0,\n",
    "            false,\n            0,\n            Some(origin_name.into()),\n            false,\n",
    "            false,\n            false,\n            None,\n        );\n",
    "        v8::Script::compile(scope, source, Some(&origin))\n"
);
const ORIGIN_HELPER_SHAPE_CASES: &[(&str, &str, &str)] = &[
    (
        "fn compile_with_origin<'scope>(",
        "fn renamed_compile_with_origin<'scope>(",
        "missing-method:ScriptCompiler::compile_with_origin",
    ),
    (
        ORIGIN_HELPER_BODY,
        "",
        "v8-script-origin-helper:statement-count",
    ),
    (
        "let origin = v8::ScriptOrigin::new(",
        "v8::ScriptOrigin::new(",
        "route-statement:origin",
    ),
    (
        "let origin = v8::ScriptOrigin::new(",
        "let other = v8::ScriptOrigin::new(",
        "route-binding:origin",
    ),
    (
        ORIGIN_INITIALIZER,
        "        let origin;\n",
        "route-initializer:origin",
    ),
    (
        ORIGIN_INITIALIZER,
        "        let origin = origin_name;\n",
        "v8-script-origin-helper:origin",
    ),
    (
        "v8::ScriptOrigin::new(",
        "Some(",
        "v8-script-origin-helper:origin",
    ),
    (
        ORIGIN_COMPILE,
        "        let compiled = v8::Script::compile(scope, source, Some(&origin));",
        "v8-script-origin-helper:compile",
    ),
    (
        ORIGIN_COMPILE,
        "        v8::Script::compile(scope, source, None)",
        "v8-script-origin-helper:compile",
    ),
    (
        ORIGIN_COMPILE,
        "        source",
        "v8-script-origin-helper:compile",
    ),
];

#[test]
fn v8_route_ast_linter_rejects_origin_helper_statement_shapes() -> Result<(), String> {
    let mut sources = crate::v8_route::test_support::actual_sources()?;
    assert_eq!(
        crate::v8_compile::inspect_compiler_helper(&sources, &mut Vec::new()),
        Ok(())
    );
    let root = crate::test_support::repository_root().map_err(|error| error.to_string())?;
    let path = crate::source::RUST_INPUTS[6];
    let text = std::fs::read_to_string(root.join(path)).map_err(|error| error.to_string())?;
    let original = sources.get(path).ok_or("missing V8 source")?.ast.clone();
    for (before, after, expected) in ORIGIN_HELPER_SHAPE_CASES {
        if text.matches(before).count() != 1 {
            return Err(format!(
                "expected a unique V8 AST mutation anchor: {before}"
            ));
        }
        let changed = text.replacen(before, after, 1);
        sources.get_mut(path).ok_or("missing V8 source")?.ast =
            syn::parse_file(&changed).map_err(|error| error.to_string())?;
        assert_eq!(
            crate::v8_compile::inspect_compiler_helper(&sources, &mut Vec::new())
                .err()
                .as_deref(),
            Some(*expected)
        );
        sources.get_mut(path).ok_or("missing V8 source")?.ast = original.clone();
    }
    Ok(())
}
