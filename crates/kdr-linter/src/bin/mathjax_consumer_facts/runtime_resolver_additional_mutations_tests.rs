pub(super) const CASES: &[(&str, &str, &str)] = &[
    (
        "let env_name = Self::runtime_env_name(kind);",
        "let extra = ();\n        let env_name = Self::runtime_env_name(kind);",
        "runtime-path-resolver:loaded-statement-count",
    ),
    (
        "Ok((path, renderer))",
        "let result = Ok((path, renderer));",
        "runtime-path-resolver:loaded-result",
    ),
    (
        "let path = path.to_path_buf();\n        match kind {",
        "let path = path.to_path_buf();\n        return match kind {",
        "runtime-path-resolver:loaded-renderer",
    ),
    (
        "DiagramKind::MathJax => Box::new(if bundled {\n                MathJaxRenderer::with_bundled_runtime_path(path)\n            } else {\n                MathJaxRenderer::with_runtime_path(path)\n            }),",
        "DiagramKind::MathJax => other,",
        "runtime-path-resolver:loaded-renderer",
    ),
    (
        "DiagramKind::MathJax => Box::new(if bundled {\n                MathJaxRenderer::with_bundled_runtime_path(path)\n            } else {\n                MathJaxRenderer::with_runtime_path(path)\n            }),",
        "DiagramKind::MathJax => Box::new(other),",
        "runtime-path-resolver:loaded-renderer",
    ),
];
