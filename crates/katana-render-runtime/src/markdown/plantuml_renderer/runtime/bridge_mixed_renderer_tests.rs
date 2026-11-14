use super::super::super::super::asset::PlantUmlJarAssetOps;
use super::super::super::super::test_support::{EnvOverride, env_guard};
use super::super::super::PlantUmlJvmRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::mermaid_renderer::MermaidRenderOps;
use crate::markdown::runtime_assets::RuntimeAsset;
use crate::markdown::svg_rasterize::{RasterizedSvg, SvgRasterizeOps};
use crate::markdown::{DiagramBlock, DiagramKind, DiagramResult};

#[test]
fn mixed_mermaid_svg_plantuml_jni_svg_keeps_the_svg_route_stable() -> Result<(), String> {
    let _guard = env_guard()?;
    let _krr_jvm = EnvOverride::unset("KRR_PLANTUML_JVM");
    let _kdr_jvm = EnvOverride::unset("KDR_PLANTUML_JVM");
    let before = rasterize_mermaid()?;
    let plantuml_svg = render_plantuml_svg()?;
    assert!(!plantuml_svg.is_empty() && plantuml_svg.contains("<svg"));
    let after = rasterize_mermaid()?;

    assert_eq!((before.width, before.height), (after.width, after.height));
    assert_eq!(before.rgba, after.rgba);
    Ok(())
}

fn rasterize_mermaid() -> Result<RasterizedSvg, String> {
    let svg = render_mermaid_svg()?;
    SvgRasterizeOps::rasterize_svg(&svg, 1.0).map_err(|error| error.to_string())
}

fn render_mermaid_svg() -> Result<String, String> {
    let asset = RuntimeAsset::mermaid();
    let path = asset
        .materialize_at(asset.materialized_path())
        .map_err(|error| error.to_string())?;
    let block = DiagramBlock {
        kind: DiagramKind::Mermaid,
        source: "flowchart TD\n    A[Start] --> B[End]".to_string(),
    };
    let DiagramResult::Ok(svg) = MermaidRenderOps::render_mermaid_with_runtime_path(
        &block,
        &path,
        DiagramColorPreset::light(),
    ) else {
        return Err("Mermaid flowchart did not produce SVG".to_string());
    };
    svg.contains("<svg")
        .then_some(svg)
        .ok_or_else(|| "Mermaid flowchart SVG root is missing".to_string())
}

fn render_plantuml_svg() -> Result<String, String> {
    let jar_path = PlantUmlJarAssetOps::cache_path(None);
    let paths = super::local_runtime_paths().ok_or_else(|| {
        format!(
            "PlantUML runtime paths unavailable for {}",
            jar_path.display()
        )
    })?;
    PlantUmlJvmRuntimeOps::render_svg(
        "@startuml\nAlice -> Bob\n@enduml",
        &paths,
        &super::themed_light_style()?,
    )
}
