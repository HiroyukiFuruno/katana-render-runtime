#![cfg(test)]

use super::RUNTIME_PATH_INPUT_INDEX;
use crate::source::{OPAQUE_INPUTS, RUST_INPUTS};
use crate::test_support::Fixture;

const PLANTUML_ARM: &str = concat!(
    "            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),\n",
    "            DiagramKind::PlantUml => Ok(plantuml_cache_dir\n",
    "                .map_or_else(PlantUmlRendererOps::default_jar_path, |path| {\n",
    "                    PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)\n",
    "                })),\n",
);
const PLANTUML_BOUNDARY_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        PLANTUML_ARM,
        concat!(
            "            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),\n",
            "            DiagramKind::PlantUml => Ok(plantuml_cache_dir.map_or_else(PlantUmlRendererOps::default_jar_path)),\n"
        ),
        "runtime-path-resolver:plantuml-arm",
    ),
    (
        PLANTUML_ARM,
        concat!(
            "            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),\n",
            "            DiagramKind::PlantUml => Ok(plantuml_cache_dir),\n"
        ),
        "runtime-path-resolver:plantuml-arm",
    ),
    (
        PLANTUML_ARM,
        concat!(
            "            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),\n",
            "            DiagramKind::PlantUml => Ok(plantuml_cache_dir.map_or_else(PlantUmlRendererOps::default_jar_path, |path| { let _cache = path; })),\n"
        ),
        "runtime-path-resolver:plantuml-cache",
    ),
    (
        PLANTUML_ARM,
        concat!(
            "            DiagramKind::MathJax => MathJaxRendererOps::resolve_mathjax_js(),\n",
            "            DiagramKind::PlantUml => Ok(plantuml_cache_dir.map_or_else(PlantUmlRendererOps::default_jar_path, |path| path)),\n"
        ),
        "runtime-path-resolver:plantuml-cache",
    ),
];

#[test]
fn plantuml_dispatch_rejects_map_or_else_shape_variants() -> Result<(), Box<dyn std::error::Error>>
{
    let inputs: Vec<&str> = [RUST_INPUTS, OPAQUE_INPUTS].concat();
    let fixture = Fixture::copy_mathjax_inputs(&inputs)?;
    fixture.reject_unique_mutations(
        RUST_INPUTS[RUNTIME_PATH_INPUT_INDEX],
        PLANTUML_BOUNDARY_MUTATIONS,
    )?;
    Ok(())
}
