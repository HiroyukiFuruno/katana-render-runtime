const WARNING: &str = "Err(warning) => {\n                let error_key = format!(\"error:{}:{warning}\", digest.as_deref().unwrap_or(\"\"));\n                Ok(RenderOutputFactory::bind_resource(\n                    RenderOutputFactory::from_raw_string(\n                        input,\n                        input.source.clone(),\n                        warning,\n                        runtime,\n                    ),\n                    Some(&error_key),\n                ))\n            }";

pub(super) const CASES: &[(&str, &str, &str)] = &[
    (
        WARNING,
        "Err(warning) => other,",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        "let error_key = format!(\"error:{}:{warning}\", digest.as_deref().unwrap_or(\"\"));",
        "let error_key = format!(\"error:{}:{warning}\", digest.as_deref().unwrap_or(\"\"));\n                let _extra = ();",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        "Ok(RenderOutputFactory::bind_resource(\n                    RenderOutputFactory::from_raw_string(\n                        input,\n                        input.source.clone(),\n                        warning,\n                        runtime,\n                    ),\n                    Some(&error_key),\n                ))",
        "other",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        "if error.kind() == std::io::ErrorKind::NotFound {\n            return format!(\n                \"MathJax runtime asset is not installed: {}\",\n                self.runtime_path.display()\n            );\n        }",
        "let _missing = error.kind();",
        "backend-runtime-path:resource-errors",
    ),
    (
        "return (Err(\"MathJax source is empty\".to_string()), None);",
        "return other;",
        "backend-runtime-path:empty-source",
    ),
];
