const UTF8_ERROR_MAPPING: &str = "std::str::from_utf8(source)\n            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))";

pub(super) const LOADER_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "Self::bundled_script_text(source)",
        "Self::bundled_script_text(other)",
        "mermaid-resource-route:argument:source",
    ),
    (
        UTF8_ERROR_MAPPING,
        concat!(
            "std::str::from_utf8(source)\n",
            "            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error));"
        ),
        "mermaid-resource-route:utf8",
    ),
    (
        "BundledResource::Mermaid => RuntimeAsset::mermaid()\n                .bytes()\n                .map_err(std::io::Error::other)?,",
        "BundledResource::Mermaid => other?,",
        "mermaid-resource-route:bundled",
    ),
    (UTF8_ERROR_MAPPING, "source", "mermaid-resource-route:utf8"),
    (
        UTF8_ERROR_MAPPING,
        "std::str::from_utf8(source)\n            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))",
        "mermaid-resource-route:utf8",
    ),
    (
        UTF8_ERROR_MAPPING,
        "source.map_err(|error| error)",
        "mermaid-resource-route:utf8",
    ),
    (
        UTF8_ERROR_MAPPING,
        "std::str::from_utf8(source).map_err(std::io::Error::other)",
        "mermaid-resource-route:utf8",
    ),
    (
        UTF8_ERROR_MAPPING,
        "std::str::from_utf8(source).map(|value| value)",
        "mermaid-resource-route:utf8",
    ),
    (
        UTF8_ERROR_MAPPING,
        "std::str::from_utf8(source).map_err()",
        "mermaid-resource-route:utf8",
    ),
    (
        UTF8_ERROR_MAPPING,
        "std::str::from_utf8(source)\n            .map_err(|failure| std::io::Error::new(std::io::ErrorKind::InvalidData, failure))",
        "mermaid-resource-route:utf8",
    ),
    (
        UTF8_ERROR_MAPPING,
        "std::str::from_utf8(source)\n            .map_err(|error| std::io::Error::other(error))",
        "mermaid-resource-route:utf8",
    ),
    (
        "BundledResource::Mermaid => RuntimeAsset::mermaid()\n                .bytes()\n                .map_err(std::io::Error::other)?,",
        "BundledResource::Mermaid => RuntimeAsset::mermaid()\n                .bytes()\n                .map_err(std::io::Error::other),",
        "mermaid-resource-route:bundled",
    ),
    (
        "let source = match bundled {",
        "let source = match other {",
        "resource-loader:read-dispatch",
    ),
    (
        "None => Self::read_file(path).map(Cow::Owned),",
        "ref None => Self::read_file(path).map(Cow::Owned),",
        "mermaid-resource-route:external",
    ),
    (
        "Self::read_file(path).map(Cow::Owned)",
        "(Self::read_file)(path).map(Cow::Owned)",
        "mermaid-resource-route:callee:Self::read_file",
    ),
    (
        ".map(Cow::Borrowed)",
        ".map(Cow::Owned)",
        "resource-loader:read-dispatch",
    ),
    (
        "RuntimeAsset::mermaid()",
        "RuntimeAsset::drawio()",
        "mermaid-resource-route:bundled",
    ),
    (
        "BundledResource::Mermaid => RuntimeAsset::mermaid()",
        "BundledResource::Drawio => RuntimeAsset::mermaid()",
        "mermaid-resource-route:bundled",
    ),
];
