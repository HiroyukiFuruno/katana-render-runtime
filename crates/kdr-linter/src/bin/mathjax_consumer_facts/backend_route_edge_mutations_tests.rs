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
];
