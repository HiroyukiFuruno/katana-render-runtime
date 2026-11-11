pub(super) const RESOURCE_READER_SHAPE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "BundledResource::MathJax",
        "BundledResource::Mermaid",
        "resource-loader:mathjax-role",
    ),
    (
        "include_str!(",
        "include_bytes!(",
        "resource-loader:mathjax-source",
    ),
    (
        "return Ok(include_str!(",
        "return Err(include_str!(",
        "resource-loader:mathjax-source",
    ),
    (
        "None => Self::read_file(path).map(Cow::Owned),",
        "Some(_) => Self::read_file(path).map(Cow::Owned),",
        "resource-loader:external-role",
    ),
    (
        ".map(Cow::Owned),",
        ".map(other),",
        "resource-loader:external-role",
    ),
    (
        "let source = match bundled {",
        "let _extra = (); let source = match bundled {",
        "resource-loader:read-body",
    ),
    (
        "let source = match bundled {",
        "let source = match other {",
        "resource-loader:read-dispatch",
    ),
    (
        "return Ok(include_str!(\n                    \"diagram_runtime/generated/mathjax-runtime.min.js\"\n                ));",
        "return other;",
        "resource-loader:mathjax-source",
    ),
    (
        "return Ok(include_str!(\n                    \"diagram_runtime/generated/mathjax-runtime.min.js\"\n                ));",
        "return Ok(other);",
        "resource-loader:mathjax-source",
    ),
    (
        "return Ok(include_str!(\n                    \"diagram_runtime/generated/mathjax-runtime.min.js\"\n                ));",
        "return Ok(other);",
        "resource-loader:mathjax-source",
    ),
    (
        "\"diagram_runtime/generated/mathjax-runtime.min.js\"",
        "concat!(\"diagram_runtime/generated/mathjax-runtime.min.js\")",
        "resource-loader:mathjax-source",
    ),
];
