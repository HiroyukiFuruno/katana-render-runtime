use super::{forwarding, parameters};

const PARAMETER_NAMES: &[&str] = &["bundle", "request_json"];
const PARAMETER_CASES: &[(&str, &str, bool)] = &[
    (
        "matching parameters",
        "fn f(bundle: i32, request_json: i32) {}",
        true,
    ),
    ("wrong arity", "fn f(bundle: i32) {}", false),
    ("wrong name", "fn f(bundle: i32, payload: i32) {}", false),
    (
        "reference pattern",
        "fn f(ref bundle: i32, request_json: i32) {}",
        false,
    ),
    (
        "subpattern",
        "fn f(bundle @ _: i32, request_json: i32) {}",
        false,
    ),
];

const FORWARDING_CASES: &[(&str, &str, bool)] = &[
    (
        "matching forwarding",
        "Self::build_scripts(bundle, MERMAID_ZENUML_RUNTIME, request_json, diagram_type == MermaidDiagramType::Zenuml)",
        true,
    ),
    (
        "wrong method",
        "Self::other(bundle, MERMAID_ZENUML_RUNTIME, request_json, diagram_type == MermaidDiagramType::Zenuml)",
        false,
    ),
    (
        "wrong argument count",
        "Self::build_scripts(bundle, MERMAID_ZENUML_RUNTIME, request_json)",
        false,
    ),
    (
        "wrong argument order",
        "Self::build_scripts(request_json, MERMAID_ZENUML_RUNTIME, bundle, diagram_type == MermaidDiagramType::Zenuml)",
        false,
    ),
    (
        "wrong comparison operator",
        "Self::build_scripts(bundle, MERMAID_ZENUML_RUNTIME, request_json, diagram_type != MermaidDiagramType::Zenuml)",
        false,
    ),
];

#[test]
fn parameter_contract_checks_typed_syn_signatures() -> Result<(), syn::Error> {
    for (case, source, expected) in PARAMETER_CASES {
        let function = syn::parse_str::<syn::ItemFn>(source)?;
        assert_eq!(
            parameters(&function.sig, PARAMETER_NAMES).is_ok(),
            *expected,
            "case: {case}"
        );
    }
    Ok(())
}

#[test]
fn forwarding_contract_checks_impl_method_expressions() -> Result<(), syn::Error> {
    for (case, body, expected) in FORWARDING_CASES {
        let method = parse_build_method(body)?;
        assert_eq!(forwarding(&method).is_ok(), *expected, "case: {case}");
    }
    Ok(())
}

fn parse_build_method(body: &str) -> Result<syn::ImplItemFn, syn::Error> {
    let item =
        syn::parse_str::<syn::ItemImpl>(&format!("impl Subject {{ fn build() {{ {body} }} }}"))?;
    match item.items.into_iter().next() {
        Some(syn::ImplItem::Fn(method)) => Ok(method),
        _ => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "expected build method",
        )),
    }
}
