use super::native_width_hash;
use super::test_support::Fixture;

#[test]
fn native_width_hash_ast_linter_rejects_hash_route_mutations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_native_width_inputs(native_width_hash::HASH_INPUTS)?;
    let report = native_width_hash::inspect_root(fixture.root())?;
    assert_eq!(report["schema"], "kdr-linter/native-width-hash-facts/v1");
    assert_eq!(report["status"], "verified");
    assert_eq!(report["facts"].as_array().ok_or("facts array")?.len(), 3);
    assert_eq!(
        report["inputSha256"]
            .as_object()
            .ok_or("pins object")?
            .len(),
        3
    );
    assert_eq!(report["nativeExecutionVerified"], false);
    assert_eq!(report["performanceVerified"], false);
    reject_import_changes(&fixture)?;
    reject_hash_field_changes(&fixture)?;
    reject_svg_id_changes(&fixture)?;
    reject_render_cache_changes(&fixture)?;
    Ok(())
}

fn reject_import_changes(fixture: &Fixture) -> Result<(), Box<dyn std::error::Error>> {
    let svg = native_width_hash::HASH_INPUTS[1];
    fixture.replace(
        svg,
        "use std::hash::{Hash, Hasher};",
        "use std::hash::Hash;",
    )?;
    fixture.reject("hash-route:imports")?;
    fixture.restore(svg)?;
    Ok(())
}

fn reject_hash_field_changes(fixture: &Fixture) -> Result<(), Box<dyn std::error::Error>> {
    let helper = native_width_hash::HASH_INPUTS[0];
    fixture.replace(helper, "source: &str", "source: &String")?;
    fixture.reject("hash-fields:signature")?;
    fixture.restore(helper)?;
    fixture.replace(
        helper,
        "preset.background.hash(hasher);\n    preset.text.hash(hasher);",
        "preset.text.hash(hasher);\n    preset.background.hash(hasher);",
    )?;
    fixture.reject("hash-fields:order")?;
    fixture.restore(helper)?;
    Ok(())
}

fn reject_svg_id_changes(fixture: &Fixture) -> Result<(), Box<dyn std::error::Error>> {
    let svg = native_width_hash::HASH_INPUTS[1];
    fixture.replace(svg, "mermaid-svg-id-v1", "mermaid-svg-id-v2")?;
    fixture.reject("svg-id-hash-route:domain")?;
    fixture.restore(svg)?;
    fixture.replace(
        svg,
        "super::hash_mermaid_source_and_preset(&mut hasher, source, preset)",
        "fake_hash_mermaid_source_and_preset(&mut hasher, source, preset)",
    )?;
    fixture.reject("svg-id-hash-route:shared-fields")?;
    fixture.restore(svg)?;
    fixture.replace(
        svg,
        "super::hash_mermaid_source_and_preset(&mut hasher, source, preset)",
        "super::hash_mermaid_source_and_preset(&mut hasher, preset, source)",
    )?;
    fixture.reject("svg-id-hash-route:shared-fields")?;
    fixture.restore(svg)?;
    reject_dead_shared_hash_call(fixture, svg)?;
    reject_shadowed_source_binding(fixture, svg)?;
    Ok(())
}

fn reject_dead_shared_hash_call(
    fixture: &Fixture,
    svg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    fixture.replace(
        svg,
        "super::hash_mermaid_source_and_preset(&mut hasher, source, preset);",
        "if false { super::hash_mermaid_source_and_preset(&mut hasher, source, preset); }",
    )?;
    fixture.reject("svg-id-hash-route:shared-fields")?;
    fixture.restore(svg)?;
    Ok(())
}

fn reject_shadowed_source_binding(
    fixture: &Fixture,
    svg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    fixture.replace(
        svg,
        "super::hash_mermaid_source_and_preset(&mut hasher, source, preset)",
        "super::hash_mermaid_source_and_preset(&mut hasher, { let source = \"shadow\"; source }, preset)",
    )?;
    fixture.reject("svg-id-hash-route:shared-fields")?;
    fixture.restore(svg)?;
    Ok(())
}

fn reject_render_cache_changes(fixture: &Fixture) -> Result<(), Box<dyn std::error::Error>> {
    let cache = native_width_hash::HASH_INPUTS[2];
    fixture.replace(
        cache,
        "MERMAID_RENDER_CACHE_SCHEMA.hash(&mut hasher);\n        MERMAID_RUNTIME_BUNDLE_CHECKSUMS.hash(&mut hasher);",
        "MERMAID_RUNTIME_BUNDLE_CHECKSUMS.hash(&mut hasher);\n        MERMAID_RENDER_CACHE_SCHEMA.hash(&mut hasher);",
    )?;
    fixture.reject("render-cache-hash-route:prefix")?;
    fixture.restore(cache)?;
    fixture.replace(cache, "katana_mermaid_cache", "forged_mermaid_cache")?;
    fixture.reject("render-cache-hash-route:path")?;
    fixture.restore(cache)?;
    fixture.replace(
        cache,
        "preset.dark_mode.hash(&mut hasher);",
        "// missing dark mode hash",
    )?;
    fixture.reject("render-cache-hash-route:statement-count")?;
    fixture.restore(cache)?;
    fixture.replace(cache, "mermaid_cache_extension", "forged_cache_extension")?;
    fixture.reject("render-cache-hash-route:filename")?;
    Ok(())
}
