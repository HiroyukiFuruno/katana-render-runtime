pub(super) const BUNDLE_STRUCTURE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "pub(super) fn build<'a>(",
        "pub(super) fn renamed_build<'a>(",
        "missing-method:MathJaxRuntimeScripts::build",
    ),
    (
        "fn runtime_source(",
        "fn renamed_runtime_source(",
        "missing-method:MathJaxRuntimeScripts::runtime_source",
    ),
    (
        "format!(\"katanaRunMathJaxRuntime({request_json});\")",
        "format!(\"katanaRunMathJaxRuntime({request_json});\";)",
        "global-invocation:arguments",
    ),
    (
        "const MATHJAX_RUNTIME: &str = include_str!(\"../diagram_runtime/generated/mathjax-runtime.min.js\");",
        "const MATHJAX_RUNTIME: &str = \"not-generated\";",
        "include-str-generated:shape",
    ),
    (
        "include_str!(\"../diagram_runtime/generated/mathjax-runtime.min.js\")",
        "include_bytes!(\"../diagram_runtime/generated/mathjax-runtime.min.js\")",
        "include-str-generated:path",
    ),
    (
        "include_str!(\"../diagram_runtime/generated/mathjax-runtime.min.js\")",
        "include_str!(MATHJAX_RUNTIME)",
        "include-str-generated:literal",
    ),
    (
        "MATHJAX_RUNTIME\n    }",
        "format!(\"{MATHJAX_RUNTIME}\")\n    }",
        "include-str-generated:source-function",
    ),
    (
        "        vec![\n            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source),",
        "        vec![\n            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source),\n            DiagramRuntimeScript::owned(\"extra.js\", runtime_source),",
        "script-order:count",
    ),
    (
        "            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source),\n            DiagramRuntimeScript::owned(\n                \"render-mathjax.js\",",
        "            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", String::new()),\n            DiagramRuntimeScript::owned(\n                \"render-mathjax.js\",",
        "script-order:source",
    ),
    (
        "DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source)",
        "DiagramRuntimeScript::other(\"mathjax-runtime.min.js\", runtime_source)",
        "script-order:callee",
    ),
    (
        "DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source)",
        "DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\")",
        "script-order:callee",
    ),
    (
        "vec![\n            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source),",
        "scripts![\n            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source),",
        "script-order:macro",
    ),
    (
        "vec![\n            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source),\n            DiagramRuntimeScript::owned(\n                \"render-mathjax.js\",\n                format!(\"katanaRunMathJaxRuntime({request_json});\"),\n            ),\n        ]",
        "vec![DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source); 2]",
        "script-order:array",
    ),
    (
        "DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source)",
        "runtime_source",
        "script-order:entry",
    ),
    (
        "DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source)",
        "(DiagramRuntimeScript::owned)(\"mathjax-runtime.min.js\", runtime_source)",
        "script-order:callee",
    ),
    (
        "DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source)",
        "DiagramRuntimeScript::borrowed(\"wrong.js\", runtime_source)",
        "script-order:name",
    ),
    (
        "DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source)",
        "DiagramRuntimeScript::borrowed(runtime_name, runtime_source)",
        "script-order:name",
    ),
    (
        "format!(\"katanaRunMathJaxRuntime({request_json});\")",
        "format!(42)",
        "global-invocation:template",
    ),
];
