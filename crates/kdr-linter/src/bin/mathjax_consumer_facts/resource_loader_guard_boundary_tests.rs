use super::{require_load_branches, require_load_result, require_owned_rejection};

fn assert_rejected<T: syn::parse::Parse>(
    sources: &[&str],
    validate: fn(&T) -> Result<(), String>,
    expected: &str,
) -> Result<(), syn::Error> {
    for source in sources {
        let expression = syn::parse_str::<T>(source)?;
        assert_eq!(validate(&expression).err().as_deref(), Some(expected));
    }
    Ok(())
}

#[test]
fn load_result_rejects_noncanonical_ast_shapes() -> Result<(), Box<dyn std::error::Error>> {
    assert_rejected(
        &[
            "source",
            "Some(LoadedResource { source })",
            "Ok(LoadedResource { source: other })",
            "Ok(LoadedResource { source, ..other })",
            "Ok(LoadedResource { source }, other)",
        ],
        require_load_result,
        "resource-loader:result",
    )?;
    let valid = syn::parse_str("Ok(LoadedResource { source })")?;
    assert!(require_load_result(&valid).is_ok());
    Ok(())
}

#[test]
fn loader_rejects_missing_or_conditional_file_branch() -> Result<(), Box<dyn std::error::Error>> {
    assert_rejected(
        &[
            "if bundled.is_some() { () }",
            "if bundled.is_some() { () } else if another { () } else { () }",
        ],
        require_load_branches,
        "resource-loader:source",
    )?;
    Ok(())
}

#[test]
fn bundled_owned_resource_must_return_the_precise_error() -> Result<(), Box<dyn std::error::Error>>
{
    assert_rejected(
        &[
            "Err(std::io::Error::other(\"bundled resource is not static\"))",
            "return Err(std::io::Error::other(\"different error\"))",
            "{ let result = Err(std::io::Error::other(\"bundled resource is not static\")); result }",
        ],
        require_owned_rejection,
        "resource-loader:source",
    )?;
    let valid =
        syn::parse_str("return Err(std::io::Error::other(\"bundled resource is not static\"))")?;
    assert!(require_owned_rejection(&valid).is_ok());
    Ok(())
}
