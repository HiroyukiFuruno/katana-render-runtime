use super::super::consumer;

#[path = "consumer_binding_contract_tests.rs"]
mod binding_contract_tests;

#[test]
fn consumer_local_guard_reports_missing_statement_and_initializer()
-> Result<(), Box<dyn std::error::Error>> {
    for (source, expected) in [
        ("{ render(source); }", "route-statement"),
        ("{ let result; }", "route-initializer"),
    ] {
        let block = syn::parse_str::<syn::Block>(source)?;
        assert_eq!(consumer::local(&block, 0).err().as_deref(), Some(expected));
    }
    Ok(())
}

#[test]
fn consumer_argument_list_guard_checks_order_and_count() -> Result<(), Box<dyn std::error::Error>> {
    let block = syn::parse_str::<syn::Block>("{ let result = render(source, preset); }")?;
    let expression = consumer::local(&block, 0)?;
    for (names, expected) in [
        (&["source", "preset"][..], None),
        (&["preset", "source"][..], Some("route:argument:preset")),
        (&["source"][..], Some("route:argument-count")),
    ] {
        assert_eq!(
            consumer::require_args(expression, names, "route")
                .err()
                .as_deref(),
            expected
        );
    }
    Ok(())
}

#[test]
fn consumer_argument_guards_reject_non_call_expressions() -> Result<(), Box<dyn std::error::Error>>
{
    let expression = syn::parse_str::<syn::Expr>("request")?;
    assert_eq!(
        consumer::require_args(&expression, &["source"], "route")
            .err()
            .as_deref(),
        Some("route:call")
    );
    assert_eq!(
        consumer::require_call_arg(&expression, 0, "source", "route")
            .err()
            .as_deref(),
        Some("route:call")
    );
    Ok(())
}

#[test]
fn consumer_single_argument_guard_checks_position() -> Result<(), Box<dyn std::error::Error>> {
    for (source, index, name, expected) in [
        (
            "{ let result = render(source, preset); }",
            0,
            "source",
            None,
        ),
        (
            "{ let result = render(source); }",
            1,
            "scripts",
            Some("route:argument:scripts"),
        ),
    ] {
        let block = syn::parse_str::<syn::Block>(source)?;
        let expression = consumer::local(&block, 0)?;
        assert_eq!(
            consumer::require_call_arg(expression, index, name, "route")
                .err()
                .as_deref(),
            expected
        );
    }
    Ok(())
}
