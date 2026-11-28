pub(in super::super) const SHARED_RESOURCE_LOADER_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "ResourceSource::File { snapshot, .. } => Some(snapshot.digest())",
        "ResourceSource::File { text, .. } => Some(snapshot.digest())",
        "resource-loader:accessors",
    ),
    (
        "let source = if let Some(kind) = bundled {",
        "let extra = ();\n        let source = if let Some(kind) = bundled {",
        "resource-loader:load-body",
    ),
    (
        "match &self.source {\n            ResourceSource::Static(text) => text,\n            ResourceSource::File { text, .. } => text.as_str(),\n        }",
        "self.source",
        "resource-loader:accessors",
    ),
    (
        "Some(snapshot.digest())",
        "snapshot.digest()",
        "resource-loader:accessors",
    ),
    (
        "Some(snapshot.digest())",
        "Some(snapshot.digest(), other)",
        "resource-loader:accessors",
    ),
    (
        "Ok(LoadedResource { source })",
        "Ok(LoadedResource { source });",
        "resource-loader:tail",
    ),
    (
        "let source = if let Some(kind) = bundled {\n            ResourceSource::Static(Self::read_bundled(kind)?)\n        } else {\n            let snapshot = ResourceBytes::snapshot(path)?;\n            let text = snapshot.text_handle()?;\n            ResourceSource::File { snapshot, text }\n        };",
        "let source = ResourceSource::Static(\"\");",
        "resource-loader:source",
    ),
    (
        "Self::load(path, None).map(|resource| resource.text().to_owned())",
        "Self::load(other, None).map(|resource| resource.text().to_owned())",
        "resource-loader:external-read",
    ),
    (
        "Self::load(path, None).map(|resource| resource.text().to_owned())",
        "Self::load(path, None).map(|_| String::new())",
        "resource-loader:external-read",
    ),
    (
        "let snapshot = ResourceBytes::snapshot(path)?;",
        "let snapshot = ResourceBytes::snapshot(other)?;",
        "resource-loader:source",
    ),
    (
        "if let Some(kind) = bundled {",
        "if let Some(_kind) = bundled {",
        "resource-loader:source",
    ),
    (
        "ResourceSource::Static(Self::read_bundled(kind)?)",
        "ResourceSource::File { snapshot, text }",
        "resource-loader:source",
    ),
    (
        "ResourceSource::Static(Self::read_bundled(kind)?)",
        "ResourceSource::Static(Self::read_bundled(BundledResource::Drawio)?)",
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
        "None => Self::read_file(path).map(Cow::Owned),",
        "None => Self::read_file(other).map(Cow::Owned),",
        "resource-loader:external-role",
    ),
    (
        "ResourceSource::Static(text) => text,",
        "ResourceSource::Static(text) => other,",
        "resource-loader:accessors",
    ),
    (
        "ResourceSource::Static(_) => None,",
        "ResourceSource::Static(_) => Some(other),",
        "resource-loader:accessors",
    ),
    (
        "ResourceSource::File { text, .. } => text.as_str(),",
        "ResourceSource::File { text, .. } => other.as_str(),",
        "resource-loader:accessors",
    ),
    (
        "ResourceSource::File { snapshot, text }",
        "ResourceSource::File { snapshot, text: other }",
        "resource-loader:source",
    ),
];
