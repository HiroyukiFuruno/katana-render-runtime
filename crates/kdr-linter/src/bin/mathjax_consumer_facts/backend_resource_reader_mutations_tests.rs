pub(super) const RESOURCE_READER_SHAPE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "pub(crate) fn read(\n        path: &Path,",
        "pub(crate) fn renamed_read(\n        path: &Path,",
        "missing-method:RuntimeResourceLoader::read",
    ),
    (
        "fn read_bundled(bundled: BundledResource)",
        "fn renamed_read_bundled(bundled: BundledResource)",
        "missing-method:RuntimeResourceLoader::read_bundled",
    ),
    (
        "match bundled {\n            Some(kind) => Self::read_bundled(kind).map(Cow::Borrowed),",
        "match bundled {\n            Some(kind) => Self::read_bundled(kind).map(Cow::Borrowed),\n            Some(other) => other,",
        "resource-loader:read-dispatch",
    ),
    (
        "match bundled {\n            Some(kind) =>",
        "let _audit = ();\n        match bundled {\n            Some(kind) =>",
        "resource-loader:read-body",
    ),
    (
        "Some(kind) => Self::read_bundled(kind).map(Cow::Borrowed),",
        "Some(ref kind) => Self::read_bundled(kind).map(Cow::Borrowed),",
        "resource-loader:read-dispatch",
    ),
    (
        "Some(kind) => Self::read_bundled(kind).map(Cow::Borrowed),",
        "Some(kind) => { Self::read_bundled(kind).map(Cow::Borrowed) },",
        "resource-loader:read-dispatch",
    ),
    (
        "let source = match bundled {",
        "let renamed = match bundled {",
        "route-binding:source",
    ),
    (
        "let source = match bundled {",
        "let source = other; match bundled {",
        "resource-loader:read-body",
    ),
    (
        "let source = match bundled {\n            BundledResource::Mermaid => RuntimeAsset::mermaid()\n                .bytes()\n                .map_err(std::io::Error::other)?,\n            BundledResource::Drawio => RuntimeAsset::drawio()\n                .bytes()\n                .map_err(std::io::Error::other)?,\n            BundledResource::MathJax => {\n                return Ok(include_str!(\n                    \"diagram_runtime/generated/mathjax-runtime.min.js\"\n                ));\n            }\n        };",
        "let source = other;",
        "resource-loader:read-dispatch",
    ),
    (
        "return Ok(include_str!(\n                    \"diagram_runtime/generated/mathjax-runtime.min.js\"\n                ));",
        "return;",
        "resource-loader:mathjax-source",
    ),
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
        "BundledResource::MathJax => {",
        "_ => unreachable!(),\n            BundledResource::MathJax => {",
        "resource-loader:read-dispatch",
    ),
    (
        "BundledResource::MathJax => {\n                return Ok(include_str!(\n                    \"diagram_runtime/generated/mathjax-runtime.min.js\"\n                ));\n            }",
        "BundledResource::MathJax => other",
        "resource-loader:mathjax-source",
    ),
    (
        "BundledResource::MathJax => {\n                return Ok(include_str!(",
        "BundledResource::MathJax => {\n                let _audit = ();\n                return Ok(include_str!(",
        "resource-loader:mathjax-source",
    ),
    (
        "\"diagram_runtime/generated/mathjax-runtime.min.js\"",
        "concat!(\"diagram_runtime/generated/mathjax-runtime.min.js\")",
        "resource-loader:mathjax-source",
    ),
];
