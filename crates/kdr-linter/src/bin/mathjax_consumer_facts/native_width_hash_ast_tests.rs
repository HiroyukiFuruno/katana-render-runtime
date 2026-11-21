use super::{
    extension_call, finish_call, named_value, plugin_key_signature, require_cache_filename,
    require_cache_path, require_format_return, string_literal, type_path_is,
};

#[test]
fn rejects_signature_ast_shapes() -> Result<(), String> {
    let method: syn::ImplItemFn =
        syn::parse_str("fn route(self, source: str) {}").map_err(|e| e.to_string())?;
    assert_eq!(
        super::require_svg_id_signature(&method).err().as_deref(),
        Some("svg-id-hash-route:signature")
    );

    let method: syn::ImplItemFn =
        syn::parse_str("fn route(source: &str, preset: &DiagramColorPreset) {}")
            .map_err(|e| e.to_string())?;
    assert_eq!(
        super::require_svg_id_signature(&method).err().as_deref(),
        Some("svg-id-hash-route:signature")
    );

    let receiver: syn::FnArg = syn::parse_str("&self").map_err(|e| e.to_string())?;
    assert!(!named_value(&receiver, "mode", "DiagramRuntimeMode"));
    assert!(!plugin_key_signature(&receiver));
    Ok(())
}

#[test]
fn rejects_expression_ast_shapes() -> Result<(), String> {
    let statement: syn::Stmt = syn::parse_str("let value = 1;").map_err(|e| e.to_string())?;
    assert!(require_format_return(&statement, "x", "value", "format-shape").is_err());
    assert!(require_cache_path(&statement).is_err());
    let wrong_format: syn::Stmt =
        syn::parse_str("format!(\"wrong\", value.finish());").map_err(|e| e.to_string())?;
    assert_eq!(
        require_format_return(&wrong_format, "expected", "value", "format-shape")
            .err()
            .as_deref(),
        Some("format-shape")
    );
    Ok(())
}

#[test]
fn rejects_expression_predicate_shapes() -> Result<(), String> {
    let wrong_filename: syn::ExprMacro =
        syn::parse_str("format!(\"wrong\", hasher.finish(), mode.mermaid_cache_extension())")
            .map_err(|e| e.to_string())?;
    assert!(require_cache_filename(&wrong_filename).is_err());
    assert!(!type_path_is(
        &syn::parse_str("&str").map_err(|e| e.to_string())?,
        "str"
    ));
    assert!(!string_literal(
        &syn::parse_str("1").map_err(|e| e.to_string())?,
        "1"
    ));
    assert!(!finish_call(
        &syn::parse_str("value").map_err(|e| e.to_string())?,
        "value.finish()"
    ));
    assert!(!extension_call(
        &syn::parse_str("value").map_err(|e| e.to_string())?
    ));
    Ok(())
}
