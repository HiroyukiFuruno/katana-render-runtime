use super::{
    Fixture, INPUTS, LIVE_CALLER_BINDING_MUTATIONS, REALM_MUTATIONS, mermaid_classic_scripts,
    reject_mutations,
};

const GEOMETRY_INSTALL_REJECTION: &str = "mermaid-classic-realm:geometry-install";
const GEOMETRY_BRIDGE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "        diagram_geometry_bridge::install(scope)?;\n",
        "        let missing_geometry_install = ();\n",
        GEOMETRY_INSTALL_REJECTION,
    ),
    (
        "        diagram_font_bridge::install(scope)?;\n        diagram_geometry_bridge::install(scope)?;",
        "        diagram_geometry_bridge::install(scope)?;\n        diagram_font_bridge::install(scope)?;",
        "mermaid-classic-realm:callee:diagram_font_bridge::install",
    ),
    (
        "diagram_geometry_bridge::install(scope)?;",
        "diagram_geometry_bridge::install(handle_scope)?;",
        GEOMETRY_INSTALL_REJECTION,
    ),
];

#[test]
fn mermaid_classic_ast_linter_rejects_caller_and_realm_mutations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_mermaid_classic_inputs(INPUTS)?;
    mermaid_classic_scripts::inspect_root(fixture.root())?;
    reject_mutations(&fixture, INPUTS[1], LIVE_CALLER_BINDING_MUTATIONS)?;
    reject_mutations(&fixture, INPUTS[2], REALM_MUTATIONS)?;
    reject_mutations(&fixture, INPUTS[2], GEOMETRY_BRIDGE_MUTATIONS)?;
    Ok(())
}
