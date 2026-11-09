pub(super) const RENDER_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "fn render_mermaid_with_origin(",
        "fn renamed_render_mermaid_with_origin(",
        "missing-method:MermaidRenderOps::render_mermaid_with_origin",
    ),
    (
        "fn render_mermaid_with_loaded(",
        "fn renamed_render_mermaid_with_loaded(",
        "missing-method:MermaidRenderOps::render_mermaid_with_loaded",
    ),
    (
        "fn render_svg_with_bundle(",
        "fn renamed_render_svg_with_bundle(",
        "missing-method:MermaidRenderOps::render_svg_with_bundle",
    ),
    (
        "mod render_instance;",
        "mod other_instance;",
        "mermaid-resource-route:instance-module",
    ),
    (
        "#[path = \"render_instance.rs\"]",
        "#[path = \"other_instance.rs\"]",
        "mermaid-resource-route:instance-module",
    ),
    (
        "runtime_resource_loader::load(\n            mermaid_js,\n            bundled.then_some(BundledResource::Mermaid),\n        )",
        "other",
        "mermaid-resource-route:load",
    ),
    (
        "return Self::error(block, format!(\"Failed to read Mermaid.js bundle: {error}\"));",
        "let ignored = error;",
        "mermaid-resource-route:load",
    ),
    (
        "let mode = DiagramRuntimeMode::current();",
        "let ignored = (); let mode = DiagramRuntimeMode::current();",
        "mermaid-resource-route:loaded-shape",
    ),
    (
        "Ok(bundle) => bundle,",
        "Ok((bundle, x)) => bundle,",
        "mermaid-resource-route:load",
    ),
    (
        "bundle.digest()",
        "bundle.digest(1)",
        "mermaid-resource-route:cache-digest",
    ),
    (
        "{error}\"));",
        "{other}\"));",
        "mermaid-resource-route:load",
    ),
    (
        "return Self::error(block, format!(\"Failed to read Mermaid.js bundle: {error}\"));",
        "return other;",
        "mermaid-resource-route:load",
    ),
    (
        "Self::render_svg_with_bundle(block, bundle.text(), preset, &cache_file)",
        "other",
        "mermaid-resource-route:render",
    ),
    (
        "Ok(bundle) => bundle,",
        "Ok(bundle) => other,",
        "mermaid-resource-route:load",
    ),
    (
        "Self::error(block, format!(\"Failed to read Mermaid.js bundle: {error}\"))",
        "Self::error(other, format!(\"Failed to read Mermaid.js bundle: {error}\"))",
        "mermaid-resource-route:load",
    ),
    (
        "let mut cache_file = Self::cache_file_path",
        "let cache_file = Self::cache_file_path",
        "mermaid-resource-route:cache-binding",
    ),
    (
        "runtime_resource_loader::load(",
        "runtime_resource_loader::other(",
        "mermaid-resource-route:load",
    ),
    (
        "bundled.then_some(BundledResource::Mermaid)",
        "bundled.then_some(BundledResource::Drawio)",
        "mermaid-resource-route:load",
    ),
    (
        "bundle.digest()",
        "other.digest()",
        "mermaid-resource-route:cache-digest",
    ),
    (
        "cache_file.set_extension(format!(\"{digest}.{}\", mode.mermaid_cache_extension()));",
        "other.set_extension(format!(\"{digest}.{}\", mode.mermaid_cache_extension()));",
        "mermaid-resource-route:cache-digest",
    ),
    (
        "Self::error(block, format!(\"Failed to read Mermaid.js bundle: {error}\"))",
        "Other::error(block, format!(\"Failed to read Mermaid.js bundle: {error}\"))",
        "mermaid-resource-route:load",
    ),
    (
        "bundle.text()",
        "other.text()",
        "mermaid-resource-route:render",
    ),
    (
        "MermaidJsRuntimeOps::render_bundle(&block.source, bundle, preset)",
        "MermaidJsRuntimeOps::render_bundle(&block.source, other, preset)",
        "mermaid-resource-route:argument",
    ),
    (
        "let bundle = match runtime_resource_loader::load(",
        "let bundle = other; let ignored = match runtime_resource_loader::load(",
        "mermaid-resource-route:origin-shape",
    ),
    (
        "let mode = DiagramRuntimeMode::current();",
        "let mode = DiagramRuntimeMode::current(other);",
        "mermaid-resource-route:argument-count",
    ),
    (
        "cache_file.set_extension(format!(\"{digest}.{}\", mode.mermaid_cache_extension()));",
        "cache_file.set_extension(format!(\"{digest}\", mode.mermaid_cache_extension()));",
        "mermaid-resource-route:cache-digest",
    ),
    (
        "Self::render_svg_with_bundle(block, bundle.text(), preset, &cache_file)",
        "Self::render_svg_with_bundle(block, bundle.text(), preset, &other)",
        "mermaid-resource-route:render",
    ),
    (
        "MermaidJsRuntimeOps::render_bundle(&block.source, bundle, preset)",
        "MermaidJsRuntimeOps::render_bundle(&other.source, bundle, preset)",
        "mermaid-resource-route:argument",
    ),
];
