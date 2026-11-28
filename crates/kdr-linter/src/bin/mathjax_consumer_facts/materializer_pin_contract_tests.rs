use super::{HELPER, OPAQUE_INPUTS, RUST_INPUTS, fixture, inspect_root, legacy_readback};

#[test]
fn mathjax_shared_readback_ast_linter_binds_current_and_legacy_routes()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    for legacy in [false, true] {
        if legacy {
            legacy_readback(&fixture, RUST_INPUTS[0])?;
        }
        let report = inspect_root(fixture.root())?;
        assert_fact_ids(&report)?;
        assert_input_pins(&report)?;
        assert_eq!(report["generatedGlobalVerified"], false);
        assert_eq!(report["overrideContentVerified"], false);
    }
    Ok(())
}

fn assert_fact_ids(report: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    let facts = report["facts"].as_array().ok_or("facts")?;
    let fact_ids = facts
        .iter()
        .map(|fact| fact["id"].as_str().ok_or("fact id"))
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    let expected_ids = [
        "backend-render-entrypoint",
        "backend-runtime-path",
        "default-materialize",
        "default-materializer",
        "explicit-override",
        "global-invocation",
        "include-str-generated",
        "read-render-route",
        "runtime-path-resolver",
        "script-order",
        "v8-compile-order",
        "v8-script-origin-helper",
        "v8-script-private-module",
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(facts.len(), expected_ids.len());
    assert_eq!(fact_ids, expected_ids);
    Ok(())
}

fn assert_input_pins(report: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    let pins = report["inputSha256"].as_object().ok_or("inputs")?;
    let actual = pins
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let expected = RUST_INPUTS
        .iter()
        .chain(OPAQUE_INPUTS)
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual, expected);
    for path in [
        HELPER,
        "crates/katana-render-runtime/src/markdown/runtime_resource_loader.rs",
        "crates/katana-render-runtime/src/markdown/mermaid_renderer/render.rs",
        "crates/katana-render-runtime/src/renderer/backends/mermaid.rs",
    ] {
        assert!(pins.contains_key(path));
    }
    Ok(())
}
