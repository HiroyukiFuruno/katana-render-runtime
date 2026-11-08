pub(super) const CASES: &[(&str, &str, &str)] = &[
    (
        "if block.source.trim().is_empty() {",
        "if block.source.is_empty() {",
        "backend-runtime-path:empty-source",
    ),
    (
        "return (Err(\"MathJax source is empty\".to_string()), None);",
        "return (Err(\"other\".to_string()), None);",
        "backend-runtime-path:empty-source",
    ),
    (
        "return (Err(\"MathJax source is empty\".to_string()), None);",
        "return (Err(\"MathJax source is empty\".to_string()), Some(other));",
        "backend-runtime-path:empty-source",
    ),
    (
        "runtime_resource_loader::load(\n            &self.runtime_path,\n            self.bundled.then_some(BundledResource::MathJax),\n        )",
        "runtime_resource_loader::load(\n            &self.runtime_path,\n            self.bundled.then_some(BundledResource::MathJax),\n            extra,\n        )",
        "backend-runtime-path:resource-load",
    ),
    (
        ".map_err(|error| self.resource_error(error))",
        ".map_err(|error, extra| self.resource_error(error))",
        "backend-runtime-path:resource-load",
    ),
    (
        "error.kind() == std::io::ErrorKind::NotFound",
        "error.kind(extra) == std::io::ErrorKind::NotFound",
        "backend-runtime-path:resource-errors",
    ),
    (
        "let resource = match self.read_resource() {\n            Ok(resource) => resource,\n            Err(error) => return (Err(error), None),\n        };",
        "let resource = self.read_resource();",
        "backend-runtime-path:resource-load",
    ),
    (
        "match self.read_resource() {",
        "match self.read_resource(extra) {",
        "backend-runtime-path:resource-load",
    ),
    (
        "\n        runtime_resource_loader::load(",
        "\n        let _resource_context = ();\n        runtime_resource_loader::load(",
        "backend-runtime-path:resource-load",
    ),
    (
        "runtime_resource_loader::load(",
        "self.resource_loader(",
        "backend-runtime-path:resource-load",
    ),
    (
        "if error.kind() == std::io::ErrorKind::NotFound {",
        "let _error_context = ();\n        if error.kind() == std::io::ErrorKind::NotFound {",
        "backend-runtime-path:resource-errors",
    ),
    (
        "return format!(\n                \"MathJax runtime asset is not installed: {}\",",
        "let _missing_context = ();\n            return format!(\n                \"MathJax runtime asset is not installed: {}\",",
        "backend-runtime-path:resource-errors",
    ),
    (
        "format!(\n            \"MathJax runtime asset could not be read: {}: {error}\",\n            self.runtime_path.display()\n        )",
        "let _read_error = format!(\n            \"MathJax runtime asset could not be read: {}: {error}\",\n            self.runtime_path.display()\n        );",
        "backend-runtime-path:resource-errors",
    ),
    (
        "return format!(\n                \"MathJax runtime asset is not installed: {}\",\n                self.runtime_path.display()\n            );",
        "return self.runtime_path.display().to_string();",
        "backend-runtime-path:resource-errors",
    ),
    (
        "if block.source.trim().is_empty() {\n            return (Err(\"MathJax source is empty\".to_string()), None);\n        }",
        "let _empty_source = block.source.trim().is_empty();",
        "backend-runtime-path:empty-source",
    ),
    (
        "return (Err(\"MathJax source is empty\".to_string()), None);",
        "let _empty_result = (Err(\"MathJax source is empty\".to_string()), None);",
        "backend-runtime-path:empty-source",
    ),
    (
        "Err(\"MathJax source is empty\".to_string())",
        "other",
        "backend-runtime-path:empty-source",
    ),
    (
        "let (result, digest) = self.render_block(input);",
        "self.render_block(input);",
        "backend-runtime-path:render-dispatch",
    ),
    (
        "Err(warning) => {",
        "Err(other) => {",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        "let error_key = format!(\"error:{}:{warning}\", digest.as_deref().unwrap_or(\"\"));",
        "let error_key = format!(\"error:{}:{warning}\", digest.as_deref().unwrap_or(\"missing\"));",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        "Ok(RenderOutputFactory::bind_resource(\n                    RenderOutputFactory::from_raw_string(",
        "Some(RenderOutputFactory::bind_resource(\n                    RenderOutputFactory::from_raw_string(",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        "RenderOutputFactory::from_raw_string(\n                        input,\n                        input.source.clone(),\n                        warning,\n                        runtime,\n                    )",
        "RenderOutputFactory::from_raw_string(input, input.source.clone(), warning, other)",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        "crate::markdown::DiagramResult::Ok(svg),",
        "crate::markdown::DiagramResult::Ok(other),",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        ".map(|output| RenderOutputFactory::bind_resource(output, digest.as_deref()))",
        ".and_then(|output| RenderOutputFactory::bind_resource(output, digest.as_deref()))",
        "backend-runtime-path:render-arm-flow",
    ),
    (
        "let (result, digest) = self.render_block(input);",
        "let (result, digest) = self.render_block(other);",
        "backend-runtime-path:render-dispatch",
    ),
];
