use super::{
    MATHJAX_RENDERER_INDEX, RUST_INPUTS, inspect_root, mathjax_fixture,
    reject_generated_include_path_change, reject_parser_pin_change, repository_root,
};
use std::fs;

#[test]
fn mathjax_consumer_facts_ast_linter_rejects_non_utf8_rust_source()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    let renderer = fixture.root().join(RUST_INPUTS[MATHJAX_RENDERER_INDEX]);
    fs::write(renderer, [0xff])?;
    let Err(error) = inspect_root(fixture.root()) else {
        return Err("invalid UTF-8 Rust source was accepted".into());
    };
    assert_eq!(
        error,
        format!("invalid-utf8:{}", RUST_INPUTS[MATHJAX_RENDERER_INDEX])
    );
    Ok(())
}

#[test]
fn mathjax_consumer_facts_ast_linter_rejects_parser_pin_and_generated_include_drift()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = repository_root()?;
    let fixture = mathjax_fixture()?;
    assert_eq!(inspect_root(fixture.root())?["status"], "verified");
    reject_parser_pin_change(&fixture, &repository)?;
    reject_generated_include_path_change(&fixture, &repository)?;
    assert_eq!(inspect_root(fixture.root())?["status"], "verified");
    Ok(())
}

#[test]
fn render_request_and_response_call_bindings_are_checked() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = mathjax_fixture()?;
    fixture.reject_unique_mutations(
        RUST_INPUTS[1],
        &[
            (
                "MathJaxRenderRequest::new(source, preset, display)",
                "MathJaxRenderRequest::new(other, preset, display)",
                "read-render-route:argument:source",
            ),
            (
                "parse_response(&output)?",
                "other_response(&output)?",
                "read-render-route:callee:parse_response",
            ),
            (
                "MathJaxRenderRequest::new(source, preset, display)",
                "other_request(source, preset, display)",
                "read-render-route:binding:request",
            ),
        ],
    )
}

#[test]
fn shared_reader_rejects_invalid_dispatch_shapes() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    fixture.reject_unique_mutations(RUST_INPUTS[9], &[
        ("let source = match bundled {\n            Some(BundledResource::Mermaid) => RuntimeAsset::mermaid()\n                .bytes()\n                .map_err(std::io::Error::other)?,\n            Some(BundledResource::Drawio) => RuntimeAsset::drawio()\n                .bytes()\n                .map_err(std::io::Error::other)?,\n            Some(BundledResource::MathJax) => {\n                return Ok(Cow::Borrowed(include_str!(\n                    \"diagram_runtime/generated/mathjax-runtime.min.js\"\n                )));\n            }\n            None => return Self::read_file(path).map(Cow::Owned),\n        };", "let source = other;", "resource-loader:read-dispatch"),
        ("Some(BundledResource::Drawio) => RuntimeAsset::drawio()\n                .bytes()\n                .map_err(std::io::Error::other)?,", "", "resource-loader:read-dispatch"),
        ("Some(BundledResource::MathJax) => {\n                return Ok(Cow::Borrowed(include_str!(\n                    \"diagram_runtime/generated/mathjax-runtime.min.js\"\n                )));\n            }", "Some(BundledResource::MathJax) => other,", "resource-loader:mathjax-source"),
        ("Some(BundledResource::MathJax) => {\n                return Ok(Cow::Borrowed(include_str!(\n                    \"diagram_runtime/generated/mathjax-runtime.min.js\"\n                )));\n            }", "Some(BundledResource::MathJax) => { other },", "resource-loader:mathjax-source"),
        ("None => return Self::read_file(path).map(Cow::Owned)", "_ => return Self::read_file(path).map(Cow::Owned)", "resource-loader:external-role"),
        ("None => return Self::read_file(path).map(Cow::Owned)", "None => return other", "resource-loader:external-role"),
    ])
}

#[test]
fn loaded_renderer_requires_a_tail_call() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    let call =
        "MathJaxJsRuntimeOps::render_bundle(&block.source, resource.text(), preset, display)";
    fixture.reject_unique_mutations(
        RUST_INPUTS[MATHJAX_RENDERER_INDEX],
        &[
            (call, "let ignored = ();", "read-render-route:loaded-call"),
            (call, "resource", "read-render-route:loaded-call"),
        ],
    )
}
