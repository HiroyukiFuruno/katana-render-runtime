const SVG_ID_FORMAT: &str = "format!(\"katana-mermaid-svg-{:016x}\", hasher.finish())";
const CACHE_FILENAME_JOIN: &str = ".join(\"katana_mermaid_cache\")\n            .join(format!(";

pub(super) const INPUT_SHAPE_MUTATIONS: &[(usize, &str, &str, &str)] = &[
    (
        2,
        "let mut hasher = DefaultHasher::new();",
        "let mut hasher = { use std::hash::BuildHasher; std::collections::hash_map::RandomState::new().build_hasher() };",
        "render-cache-hash-route:hasher",
    ),
    (
        0,
        "preset.background.hash(hasher);",
        "let _background = preset.background;",
        "hash-fields:order",
    ),
    (
        1,
        "DefaultHasher::new()",
        "DefaultHasher::default()",
        "svg-id-hash-route:hasher",
    ),
    (
        1,
        SVG_ID_FORMAT,
        "String::from(\"fixed-svg-id\")",
        "svg-id-hash-route:output",
    ),
    (
        1,
        SVG_ID_FORMAT,
        "stringify!(\"katana-mermaid-svg-{:016x}\", hasher.finish())",
        "svg-id-hash-route:output",
    ),
    (
        1,
        SVG_ID_FORMAT,
        "let _svg_id = format!(\"katana-mermaid-svg-{:016x}\", hasher.finish());",
        "svg-id-hash-route:output",
    ),
    (
        1,
        SVG_ID_FORMAT,
        "format!(\"katana-mermaid-svg-{:016x}\"; hasher.finish())",
        "svg-id-hash-route:output",
    ),
    (
        2,
        CACHE_FILENAME_JOIN,
        ".join(\"katana_mermaid_cache\")\n            .join(filename(",
        "render-cache-hash-route:filename",
    ),
    (
        2,
        CACHE_FILENAME_JOIN,
        ".join(\"katana_mermaid_cache\")\n            .join(stringify!(",
        "render-cache-hash-route:filename",
    ),
    (
        2,
        "std::env::temp_dir()\n            .join(\"katana_mermaid_cache\")",
        "cache_directory()",
        "render-cache-hash-route:path",
    ),
    (
        2,
        "format!(\n                \"{:016x}.{}\",\n                hasher.finish(),\n                mode.mermaid_cache_extension()\n            )",
        "format!(\n                \"{:016x}.{}\",\n                hasher.to_string(),\n                mode.mermaid_cache_extension()\n            )",
        "render-cache-hash-route:filename",
    ),
    (
        2,
        "hasher.finish(),\n                mode.mermaid_cache_extension()",
        "hasher.finish(),\n                mode.to_string()",
        "render-cache-hash-route:filename",
    ),
    (
        2,
        "mermaid_js.hash(&mut hasher);",
        "source.hash(&mut hasher);",
        "render-cache-hash-route:prefix:mermaid_js:method=hash,receiver=source,args=1",
    ),
    (
        2,
        "preset.dark_mode.hash(&mut hasher);",
        "let _ = preset.dark_mode;",
        "render-cache-hash-route:dark-mode",
    ),
    (
        2,
        "preset.dark_mode.hash(&mut hasher);",
        "preset.light_mode.hash(&mut hasher);",
        "render-cache-hash-route:dark-mode:method=hash,receiver=preset.light_mode,args=1",
    ),
];
