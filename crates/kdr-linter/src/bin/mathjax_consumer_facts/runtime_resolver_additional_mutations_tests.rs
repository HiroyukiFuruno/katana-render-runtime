const SELECTED_DISPATCH: &str = concat!(
    "match kind {\n",
    "            DiagramKind::Mermaid => MermaidBinaryOps::resolve_mermaid_js_with_env(\n",
    "                env_value,\n                MermaidBinaryOps::default_install_path(),\n            ),\n",
    "            DiagramKind::Drawio => DrawioRendererOps::resolve_drawio_js_with_env(\n",
    "                env_value,\n                DrawioRendererOps::default_install_path(),\n            ),\n",
    "            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js_with_env(\n",
    "                env_value,\n                MathJaxRendererOps::default_install_path(),\n            ),\n",
    "            DiagramKind::PlantUml => Ok(plantuml_cache_dir\n",
    "                .map_or_else(PlantUmlRendererOps::default_jar_path, |path| {\n",
    "                    PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)\n",
    "                })),\n        }",
);

pub(super) const CASES: &[(&str, &str, &str)] = &[
    (
        SELECTED_DISPATCH,
        "other",
        "runtime-path-resolver:loaded-path",
    ),
    (
        ".map_err(RenderError::RuntimeResolution)\n    }\n\n    fn make_renderer(",
        "\n    }\n\n    fn make_renderer(",
        "runtime-path-resolver:loaded-path",
    ),
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
