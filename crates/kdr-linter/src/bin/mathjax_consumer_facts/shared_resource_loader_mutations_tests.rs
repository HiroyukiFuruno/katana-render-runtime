pub(in super::super) const SHARED_RESOURCE_LOADER_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "Ok(LoadedResource { source })",
        "Ok(LoadedResource { source });",
        "resource-loader:tail",
    ),
    (
        "let source = if bundled.is_some() {\n            match Self::read(path, bundled)? {\n                Cow::Borrowed(text) => ResourceSource::Static(text),\n                Cow::Owned(_) => {\n                    return Err(std::io::Error::other(\"bundled resource is not static\"));\n                }\n            }\n        } else {\n            let snapshot = ResourceBytes::snapshot(path)?;\n            let text = snapshot.text_handle()?;\n            ResourceSource::File { snapshot, text }\n        };",
        "let source = ResourceSource::Static(\"\");",
        "resource-loader:source",
    ),
    (
        "crate::resource_bytes::ResourceBytes::read_text(path)",
        "crate::resource_bytes::ResourceBytes::read_text(other)",
        "resource-loader:external-read",
    ),
    (
        "let snapshot = ResourceBytes::snapshot(path)?;",
        "let snapshot = ResourceBytes::snapshot(other)?;",
        "resource-loader:source",
    ),
    (
        "if bundled.is_some() {",
        "if bundled.is_none() {",
        "resource-loader:source",
    ),
    (
        "Cow::Borrowed(text) => ResourceSource::Static(text)",
        "Cow::Borrowed(text) => ResourceSource::Static(other)",
        "resource-loader:source",
    ),
    (
        "bundled resource is not static",
        "different error",
        "resource-loader:source",
    ),
    (
        "let text = snapshot.text_handle()?;",
        "let text = other.text_handle()?;",
        "resource-loader:source",
    ),
    (
        "ResourceSource::File { snapshot, text }",
        "ResourceSource::File { snapshot: other, text }",
        "resource-loader:source",
    ),
    (
        "Ok(LoadedResource { source })",
        "Ok(LoadedResource { source: other })",
        "resource-loader:result",
    ),
    (
        "Some(snapshot.digest())",
        "Some(other.digest())",
        "resource-loader:accessors",
    ),
    (
        "None => return Self::read_file(path).map(Cow::Owned)",
        "None => return Self::read_file(other).map(Cow::Owned)",
        "resource-loader:external-role",
    ),
];
