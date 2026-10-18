use super::super::super::resolve::{PlantUmlRuntimePathOps, PlantUmlRuntimePaths};
use super::super::super::test_support::{EnvOverride, env_guard};
use super::super::PlantUmlJvmRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::plantuml_renderer::asset::PlantUmlJarAssetOps;
use crate::markdown::plantuml_renderer::theme::{
    PlantUmlRenderStyle, PlantUmlThemeConfig, PlantUmlThemeOps,
};

#[test]
fn bridge_renders_light_and_dark_svg_with_pinned_local_plantuml_runtime() -> Result<(), String> {
    let _guard = env_guard()?;
    let _krr_jvm = EnvOverride::unset("KRR_PLANTUML_JVM");
    let _kdr_jvm = EnvOverride::unset("KDR_PLANTUML_JVM");
    let Some(paths) = local_runtime_paths() else {
        return Ok(());
    };
    let light_style =
        PlantUmlThemeOps::style(DiagramColorPreset::light(), &PlantUmlThemeConfig::default());

    let svg = render_alice_sequence(&paths, &light_style)?;
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Alice"));
    let svg = render_alice_sequence(&paths, &themed_light_style()?)?;
    assert!(svg.contains("<svg"));
    let dark_style =
        PlantUmlThemeOps::style(DiagramColorPreset::dark(), &PlantUmlThemeConfig::default());
    let svg = render_alice_sequence(&paths, &dark_style)?;

    assert!(svg.contains("<svg"));
    Ok(())
}

#[test]
fn bridge_reports_invalid_diagram_descriptions() -> Result<(), String> {
    let _guard = env_guard()?;
    let _krr_jvm = EnvOverride::unset("KRR_PLANTUML_JVM");
    let _kdr_jvm = EnvOverride::unset("KDR_PLANTUML_JVM");
    let Some(paths) = local_runtime_paths() else {
        return Ok(());
    };
    let style =
        PlantUmlThemeOps::style(DiagramColorPreset::light(), &PlantUmlThemeConfig::default());

    let result = PlantUmlJvmRuntimeOps::render_svg(
        "@startuml\n!error forced test failure\n@enduml",
        &paths,
        &style,
    );

    assert!(matches!(result, Err(message) if message.contains("PlantUML render failed")));
    Ok(())
}

#[test]
fn jni_error_message_preserves_display_text() {
    assert_eq!(
        super::jni_error_message(jni::errors::Error::NullPtr("bridge test")),
        "Null pointer in bridge test"
    );
}

fn local_runtime_paths() -> Option<PlantUmlRuntimePaths> {
    let jar_path = PlantUmlJarAssetOps::cache_path(None);
    PlantUmlRuntimePathOps::resolve_paths(&jar_path, None).ok()
}

fn render_alice_sequence(
    paths: &PlantUmlRuntimePaths,
    style: &PlantUmlRenderStyle,
) -> Result<String, String> {
    PlantUmlJvmRuntimeOps::render_svg("@startuml\nAlice -> Bob\n@enduml", paths, style)
}

fn themed_light_style() -> Result<PlantUmlRenderStyle, String> {
    let themed_config = PlantUmlThemeConfig::from_value(&serde_json::json!({
        "plantuml_theme": "cyborg",
        "plantuml_theme_mode": "light",
    }))?;
    Ok(PlantUmlThemeOps::style(
        DiagramColorPreset::light(),
        &themed_config,
    ))
}
