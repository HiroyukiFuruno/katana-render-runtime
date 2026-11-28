const RENDER_MATCH: &str = "match MermaidJsRuntimeOps::render_bundle(&block.source, bundle, preset) {\n            Ok(svg) => Self::write_cached_svg(block, cache_file, svg),\n            Err(error) => Self::error(block, error),\n        }";

pub(super) const CASES: &[(&str, &str, &str)] = &[
    (
        "Err(error) => {\n                return Self::error(block, format!(\"Failed to read Mermaid.js bundle: {error}\"));\n            }",
        "Err(error) => other,",
        "mermaid-resource-route:load",
    ),
    (RENDER_MATCH, "other", "mermaid-resource-route:render-shape"),
    (
        RENDER_MATCH,
        "match MermaidJsRuntimeOps::render_bundle(&block.source, bundle, preset) {\n            Ok(svg) => Self::write_cached_svg(block, cache_file, svg),\n            Err(error) => Self::error(block, error),\n            _ => other,\n        }",
        "mermaid-resource-route:render-arms",
    ),
    (
        "if let Some(digest) = bundle.digest() {\n            cache_file.set_extension(format!(\"{digest}.{}\", mode.mermaid_cache_extension()));\n        }",
        "let _ = bundle.digest();",
        "mermaid-resource-route:cache-digest",
    ),
    (
        "cache_file.set_extension(format!(\"{digest}.{}\", mode.mermaid_cache_extension()));",
        "other;",
        "mermaid-resource-route:cache-digest",
    ),
    (
        "let bundle = match runtime_resource_loader::load(\n            mermaid_js,\n            bundled.then_some(BundledResource::Mermaid),\n        ) {\n            Ok(bundle) => bundle,\n            Err(error) => {\n                return Self::error(block, format!(\"Failed to read Mermaid.js bundle: {error}\"));\n            }\n        };",
        "let bundle = other;",
        "mermaid-resource-route:load",
    ),
];
