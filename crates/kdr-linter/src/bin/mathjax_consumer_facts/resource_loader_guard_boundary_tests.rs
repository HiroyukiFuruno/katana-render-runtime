use super::{require_load_branches, require_load_result, require_static_branch};

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
fn bundled_branch_requires_typed_helper_and_error_propagation()
-> Result<(), Box<dyn std::error::Error>> {
    assert_rejected(
        &[
            "{ ResourceSource::Static(Self::read_bundled(other)?) }",
            "{ ResourceSource::Static(Self::read(kind)?) }",
            "{ ResourceSource::Static(Self::read_bundled(kind)) }",
            "{ ResourceSource::Static(Self::read_bundled(kind)?, other) }",
        ],
        require_static_branch,
        "resource-loader:source",
    )?;
    let valid = syn::parse_str("{ ResourceSource::Static(Self::read_bundled(kind)?) }")?;
    assert!(require_static_branch(&valid).is_ok());
    Ok(())
}

#[test]
fn loader_requires_exact_bundled_binding_and_file_operations()
-> Result<(), Box<dyn std::error::Error>> {
    let canonical = "if let Some(kind) = bundled { ResourceSource::Static(Self::read_bundled(kind)?) } else { let snapshot = ResourceBytes::snapshot(path)?; let text = snapshot.text_handle()?; ResourceSource::File { snapshot, text } }";
    let validate = |source: &str| -> Result<(), Box<dyn std::error::Error>> {
        let file = syn::parse_str(&format!(
            "impl RuntimeResourceLoader {{ fn load() {{ let source = {source}; Ok(LoadedResource {{ source }}) }} }}"
        ))?;
        assert!(super::require_load(&file).is_err());
        Ok(())
    };
    for replacement in [
        canonical.replace("Some(kind) = bundled", "Some(kind) = other"),
        canonical.replace("Some(kind) = bundled", "Some(other) = bundled"),
        canonical.replace("snapshot(path)?", "snapshot(other)?"),
        canonical.replace("snapshot(path)?", "snapshot(path)"),
        canonical.replace("snapshot.text_handle()?", "snapshot.text_handle()"),
        canonical.replace("snapshot.text_handle()?", "other.text_handle()?"),
    ] {
        validate(&replacement)?;
    }
    let file = syn::parse_str(&format!(
        "impl RuntimeResourceLoader {{ fn load() {{ let source = {canonical}; Ok(LoadedResource {{ source }}) }} }}"
    ))?;
    assert!(super::require_load(&file).is_ok());
    Ok(())
}
