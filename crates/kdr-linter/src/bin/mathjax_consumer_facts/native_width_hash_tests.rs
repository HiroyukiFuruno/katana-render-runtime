use super::native_width_hash;
use super::test_support::Fixture;

#[path = "native_width_hash_contract_tests.rs"]
mod contract_tests;

const SVG_ID_SIGNATURE: &str = "fn svg_id(source: &str, preset: &DiagramColorPreset) -> String";
const CACHE_SIGNATURE: &str = "fn cache_file_path_with_zenuml_key(\n        source: &str,\n        mermaid_js: &Path,\n        preset: &DiagramColorPreset,\n        mode: DiagramRuntimeMode,\n        zenuml_plugin_key: (&str, &str),\n    ) -> PathBuf";
const SVG_SIGNATURE_MUTATIONS: &[(&str, &str)] = &[
    ("preset: &DiagramColorPreset", "preset: &Theme"),
    ("source: &str", "source: &mut str"),
    ("source: &str", "content: &str"),
    (
        "source: &str, preset: &DiagramColorPreset",
        "&self, preset: &DiagramColorPreset",
    ),
    (
        "preset: &DiagramColorPreset) -> String",
        "preset: &DiagramColorPreset, unused: bool) -> String",
    ),
    (") -> String", ") -> PathBuf"),
    (") -> String", ")"),
];
const CACHE_SIGNATURE_MUTATIONS: &[(&str, &str)] = &[
    ("source: &str", "content: &str"),
    ("source: &str", "source: &mut str"),
    ("mermaid_js: &Path", "mermaid_js: &str"),
    ("mermaid_js: &Path", "mermaid_js: Path"),
    ("mermaid_js: &Path", "mermaid_js: &std::path::Path"),
    ("mermaid_js: &Path", "mermaid_js: &&Path"),
    ("preset: &DiagramColorPreset", "preset: &Theme"),
    ("mode: DiagramRuntimeMode", "mode: bool"),
    (
        "mode: DiagramRuntimeMode",
        "runtime_mode: DiagramRuntimeMode",
    ),
    (
        "zenuml_plugin_key: (&str, &str)",
        "plugin_key: (&str, &str)",
    ),
    ("zenuml_plugin_key: (&str, &str)", "zenuml_plugin_key: &str"),
    (
        "zenuml_plugin_key: (&str, &str)",
        "zenuml_plugin_key: (&String, &str)",
    ),
    (
        "zenuml_plugin_key: (&str, &str)",
        "zenuml_plugin_key: (&str, &str, &str)",
    ),
    (
        "zenuml_plugin_key: (&str, &str),\n    ) -> PathBuf",
        "zenuml_plugin_key: (&str, &str),\n        unused: bool,\n    ) -> PathBuf",
    ),
    (") -> PathBuf", ") -> String"),
    (") -> PathBuf", ")"),
];
const INPUT_SHAPE_MUTATIONS: &[(usize, &str, &str, &str)] = &[
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
        "format!(\"katana-mermaid-svg-{:016x}\", hasher.finish())",
        "stringify!(\"katana-mermaid-svg-{:016x}\", hasher.finish())",
        "svg-id-hash-route:output",
    ),
    (
        1,
        "format!(\"katana-mermaid-svg-{:016x}\", hasher.finish())",
        "let _svg_id = format!(\"katana-mermaid-svg-{:016x}\", hasher.finish());",
        "svg-id-hash-route:output",
    ),
    (
        2,
        ".join(\"katana_mermaid_cache\")\n            .join(format!(",
        ".join(\"katana_mermaid_cache\")\n            .join(filename(",
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
        "mode.mermaid_cache_extension()",
        "mode.to_string()",
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
        "preset.light_mode.hash(&mut hasher);",
        "render-cache-hash-route:dark-mode:method=hash,receiver=preset.light_mode,args=1",
    ),
];

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
    reject_signature_mutations(&fixture)?;
    reject_input_shape_mutations(&fixture)?;
    Ok(())
}

fn reject_signature_mutations(fixture: &Fixture) -> Result<(), Box<dyn std::error::Error>> {
    check_signature_mutations(
        fixture,
        native_width_hash::HASH_INPUTS[1],
        SVG_ID_SIGNATURE,
        SVG_SIGNATURE_MUTATIONS,
        "svg-id-hash-route:signature",
    )?;
    check_signature_mutations(
        fixture,
        native_width_hash::HASH_INPUTS[2],
        CACHE_SIGNATURE,
        CACHE_SIGNATURE_MUTATIONS,
        "render-cache-hash-route:signature",
    )?;
    Ok(())
}

fn check_signature_mutations(
    fixture: &Fixture,
    path: &str,
    signature: &str,
    mutations: &[(&str, &str)],
    diagnostic: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for (original, replacement) in mutations {
        let changed_signature = signature.replace(original, replacement);
        assert_ne!(
            changed_signature, signature,
            "mutation must change signature"
        );
        fixture.replace(path, signature, &changed_signature)?;
        fixture.reject(diagnostic)?;
        fixture.restore(path)?;
    }
    Ok(())
}

fn reject_input_shape_mutations(fixture: &Fixture) -> Result<(), Box<dyn std::error::Error>> {
    for (input_index, original, replacement, diagnostic) in INPUT_SHAPE_MUTATIONS {
        let path = native_width_hash::HASH_INPUTS[*input_index];
        fixture.replace(path, original, replacement)?;
        fixture.reject(diagnostic)?;
        fixture.restore(path)?;
    }
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
