use super::matches_formatted_error;
use syn::parse::Parser;

#[test]
fn formatted_error_compares_generic_expected_arguments() -> Result<(), syn::Error> {
    let values = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
        .parse_str("\"message: {}\", actual")?;
    assert!(matches_formatted_error(&values, "message: {}", &["actual"]));
    assert!(!matches_formatted_error(&values, "message: {}", &["other"]));
    Ok(())
}
