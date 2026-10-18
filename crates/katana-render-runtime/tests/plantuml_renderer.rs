#[path = "../src/markdown/plantuml_renderer/test_env_override.rs"]
mod test_env_override;
use test_env_override::EnvOverride;

use katana_render_runtime::{
    DiagramKind, PLANTUML_DOWNLOAD_URL, PLANTUML_JAR_CHECKSUM, PLANTUML_JAR_VERSION,
    PlantUmlRenderer, PlantUmlThemeCatalog, RenderConfig, RenderContext, RenderError, RenderInput,
    RenderPolicy, Renderer, RuntimePathResolver,
};

const OFFICIAL_FIXTURES: [&str; 9] = [
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/plantuml/official/01-sequence.puml"
    ),
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/plantuml/official/02-use-case.puml"
    ),
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/plantuml/official/03-class.puml"
    ),
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/plantuml/official/04-object.puml"
    ),
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/plantuml/official/05-activity.puml"
    ),
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/plantuml/official/06-component.puml"
    ),
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/plantuml/official/07-deployment.puml"
    ),
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/plantuml/official/08-state.puml"
    ),
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/plantuml/official/09-timing.puml"
    ),
];
static PLANTUML_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn plantuml_api_exposes_available_theme_names() {
    let renderer_themes = PlantUmlRenderer::available_themes();

    assert_eq!(renderer_themes, PlantUmlThemeCatalog::names());
    assert!(renderer_themes.contains(&"cyborg"));
    assert!(renderer_themes.contains(&"black-knight"));
    assert!(renderer_themes.contains(&"spacelab"));
}

#[test]
fn plantuml_api_exposes_runtime_asset_metadata() {
    assert_eq!(PLANTUML_JAR_CHECKSUM.len(), 64);
    assert!(PLANTUML_DOWNLOAD_URL.contains(PLANTUML_JAR_VERSION));
}

#[test]
fn plantuml_cache_dir_resolves_versioned_runtime_path() -> Result<(), Box<dyn std::error::Error>> {
    let cache_dir =
        std::env::temp_dir().join(format!("krr-plantuml-cache-dir-{}", std::process::id()));

    let path = RuntimePathResolver::resolve_with_plantuml_cache_dir(
        DiagramKind::PlantUml,
        None,
        Some(cache_dir.clone()),
    )?;

    assert_eq!(
        path,
        cache_dir.join(PLANTUML_JAR_VERSION).join("plantuml.jar")
    );
    Ok(())
}

#[test]
fn plantuml_cache_env_fallback_is_publicly_resolvable() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = PLANTUML_ENV_LOCK.lock()?;
    let cache_dir =
        std::env::temp_dir().join(format!("krr-plantuml-kdr-cache-{}", std::process::id()));
    let _krr_cache = EnvOverride::unset("KRR_PLANTUML_CACHE_DIR");
    let _kdr_cache = EnvOverride::set("KDR_PLANTUML_CACHE_DIR", cache_dir.as_os_str());

    let path = RuntimePathResolver::resolve(DiagramKind::PlantUml, None)?;

    assert_eq!(
        path,
        cache_dir.join(PLANTUML_JAR_VERSION).join("plantuml.jar")
    );
    Ok(())
}

#[test]
fn plantuml_default_cache_path_falls_back_without_home_environment()
-> Result<(), Box<dyn std::error::Error>> {
    let _guard = PLANTUML_ENV_LOCK.lock()?;
    let _krr_cache = EnvOverride::unset("KRR_PLANTUML_CACHE_DIR");
    let _kdr_cache = EnvOverride::unset("KDR_PLANTUML_CACHE_DIR");
    let _home = EnvOverride::unset("HOME");
    let _user_profile = EnvOverride::unset("USERPROFILE");
    #[cfg(target_os = "windows")]
    let _local_app_data = EnvOverride::unset("LOCALAPPDATA");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _xdg_cache_home = EnvOverride::unset("XDG_CACHE_HOME");

    let path = RuntimePathResolver::resolve(DiagramKind::PlantUml, None)?;

    assert!(path.starts_with(std::env::temp_dir()), "{}", path.display());
    assert!(path.ends_with(format!("krr/plantuml/{PLANTUML_JAR_VERSION}/plantuml.jar")));
    Ok(())
}

#[test]
fn plantuml_empty_source_returns_empty_output() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = PlantUmlRenderer::with_runtime_path(missing_jar_path());
    let output = renderer.render(&input(""))?;

    assert_eq!(output.svg, "");
    assert_eq!(output.width, 0.0);
    assert_eq!(output.height, 0.0);
    Ok(())
}

#[test]
fn plantuml_rejects_non_object_vendor_config() {
    let renderer = PlantUmlRenderer::with_runtime_path(missing_jar_path());
    let result = renderer.render(&input_with_vendor_config(
        "@startuml\nAlice -> Bob\n@enduml",
        serde_json::json!("not an object"),
    ));

    assert!(matches!(
        result,
        Err(RenderError::Runtime(error)) if error.contains("invalid PlantUML config")
    ));
}

#[test]
fn plantuml_cache_dir_option_uses_configured_cache() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = PLANTUML_ENV_LOCK.lock()?;
    let _krr_jar = EnvOverride::unset("KRR_PLANTUML_JAR");
    let _kdr_jar = EnvOverride::unset("KDR_PLANTUML_JAR");
    let _plantuml_jar = EnvOverride::unset("PLANTUML_JAR");
    let _krr_cache = EnvOverride::unset("KRR_PLANTUML_CACHE_DIR");
    let _kdr_cache = EnvOverride::unset("KDR_PLANTUML_CACHE_DIR");
    let cache_dir = write_invalid_cache_jar("krr-plantuml-option-cache")?;
    let renderer = PlantUmlRenderer::with_runtime_path(RuntimePathResolver::resolve(
        DiagramKind::PlantUml,
        None,
    )?);

    let output = renderer.render(&input_with_vendor_config(
        "@startuml\nAlice -> Bob\n@enduml",
        serde_json::json!({ "plantuml_cache_dir": cache_dir.clone() }),
    ))?;
    let _ = std::fs::remove_dir_all(&cache_dir);

    assert!(output.svg.starts_with("```plantuml"));
    assert!(
        output
            .diagnostics
            .warnings
            .iter()
            .any(|warning| warning.contains("checksum mismatch"))
    );
    Ok(())
}

fn write_invalid_cache_jar(prefix: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let cache_dir = std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()));
    let cache_jar = cache_dir.join(PLANTUML_JAR_VERSION).join("plantuml.jar");
    let Some(parent) = cache_jar.parent() else {
        return Err("cache jar path has no parent".into());
    };
    std::fs::create_dir_all(parent)?;
    std::fs::write(&cache_jar, b"invalid plantuml jar")?;
    Ok(cache_dir)
}

#[test]
fn plantuml_missing_runtime_returns_raw_code_with_warning() -> Result<(), Box<dyn std::error::Error>>
{
    let renderer = PlantUmlRenderer::with_runtime_path(missing_jar_path());
    let output = renderer.render(&input("@startuml\nAlice -> Bob: hello\n@enduml"))?;

    assert!(output.svg.starts_with("```plantuml"));
    assert!(output.svg.contains("Alice -> Bob"));
    assert_eq!(output.width, 0.0);
    assert_eq!(output.height, 0.0);
    assert!(
        output.diagnostics.warnings[0].contains("plantuml-runtime-unavailable"),
        "warning should explain the fallback: {:?}",
        output.diagnostics.warnings
    );
    Ok(())
}

#[test]
fn plantuml_renders_svg_when_local_runtime_is_available() -> Result<(), Box<dyn std::error::Error>>
{
    let _guard = PLANTUML_ENV_LOCK.lock()?;
    let Some(renderer) = local_runtime_renderer()? else {
        return Ok(());
    };
    let output = renderer.render(&input("@startuml\nAlice -> Bob: hello\n@enduml"))?;

    if output.diagnostics.warnings.is_empty() {
        assert!(output.svg.contains("<svg"));
        assert!(output.svg.contains("Alice"));
        assert!(output.svg.contains("Bob"));
        assert!(output.width > 0.0);
        assert!(output.height > 0.0);
    } else {
        assert!(output.svg.starts_with("```plantuml"));
    }
    Ok(())
}

#[test]
fn plantuml_representative_fixtures_render_when_local_runtime_is_available()
-> Result<(), Box<dyn std::error::Error>> {
    let _guard = PLANTUML_ENV_LOCK.lock()?;
    let Some(renderer) = local_runtime_renderer()? else {
        return Ok(());
    };
    assert_fixtures_render(&renderer, &representative_fixtures())
}

#[test]
fn plantuml_official_fixtures_render_when_local_runtime_is_available()
-> Result<(), Box<dyn std::error::Error>> {
    let _guard = PLANTUML_ENV_LOCK.lock()?;
    let Some(renderer) = local_runtime_renderer()? else {
        return Ok(());
    };
    assert_fixtures_render(&renderer, &OFFICIAL_FIXTURES)
}

#[test]
fn plantuml_dark_mode_uses_official_color_mapper_when_local_runtime_is_available()
-> Result<(), Box<dyn std::error::Error>> {
    let _guard = PLANTUML_ENV_LOCK.lock()?;
    let Some(renderer) = local_runtime_renderer()? else {
        return Ok(());
    };
    let source = std::fs::read_to_string(representative_fixtures()[1])?;
    let output = renderer.render(&input(&source))?;
    if !output.diagnostics.warnings.is_empty() {
        return Ok(());
    }

    assert!(output.svg.contains("#1B1B1B"), "{}", output.svg);
    assert!(output.svg.contains("#313139"), "{}", output.svg);
    assert!(output.svg.contains("#E7E7E7"), "{}", output.svg);
    assert!(output.svg.contains("#2E5233"), "{}", output.svg);
    Ok(())
}

#[test]
fn plantuml_theme_option_uses_official_theme_directive_when_local_runtime_is_available()
-> Result<(), Box<dyn std::error::Error>> {
    let _guard = PLANTUML_ENV_LOCK.lock()?;
    let Some(renderer) = local_runtime_renderer()? else {
        return Ok(());
    };
    let source = std::fs::read_to_string(representative_fixtures()[1])?;
    let default_output = renderer.render(&input(&source))?;
    let themed_output = renderer.render(&input_with_theme(&source, "cyborg"))?;
    if !default_output.diagnostics.warnings.is_empty()
        || !themed_output.diagnostics.warnings.is_empty()
    {
        return Ok(());
    }

    assert_ne!(default_output.svg, themed_output.svg);
    assert_ne!(
        default_output.cache_fingerprint,
        themed_output.cache_fingerprint
    );
    assert!(themed_output.svg.contains("data-visibility-modifier"));
    Ok(())
}

#[test]
fn plantuml_invalid_source_returns_runtime_error_when_runtime_is_available()
-> Result<(), Box<dyn std::error::Error>> {
    let _guard = PLANTUML_ENV_LOCK.lock()?;
    let Some(renderer) = local_runtime_renderer()? else {
        return Ok(());
    };
    let result = renderer.render(&input("@startuml\n!error forced test failure\n@enduml"));

    assert!(matches!(
        result,
        Err(RenderError::Runtime(message)) if message.contains("PlantUML render failed")
    ));
    Ok(())
}

fn local_runtime_renderer() -> Result<Option<PlantUmlRenderer>, Box<dyn std::error::Error>> {
    let jar_path = RuntimePathResolver::resolve(DiagramKind::PlantUml, None)?;
    Ok(jar_path
        .exists()
        .then(|| PlantUmlRenderer::with_runtime_path(jar_path)))
}

fn assert_fixtures_render(
    renderer: &PlantUmlRenderer,
    fixtures: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    for &fixture in fixtures {
        let source = std::fs::read_to_string(fixture)?;
        let output = renderer.render(&input(&source))?;
        if output.diagnostics.warnings.is_empty() {
            assert!(
                output.svg.contains("<svg"),
                "fixture should render: {fixture}"
            );
            assert!(output.width > 0.0, "fixture should have width: {fixture}");
            assert!(output.height > 0.0, "fixture should have height: {fixture}");
        }
    }
    Ok(())
}

fn missing_jar_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("kdr-missing-plantuml-{}.jar", std::process::id()))
}

fn representative_fixtures() -> [&'static str; 3] {
    [
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/plantuml/representative/01-sequence.puml"
        ),
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/plantuml/representative/02-class.puml"
        ),
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/plantuml/representative/03-activity.puml"
        ),
    ]
}

fn input(source: &str) -> RenderInput {
    RenderInput {
        kind: DiagramKind::PlantUml,
        source: source.to_string(),
        config: RenderConfig::default(),
        policy: RenderPolicy::default(),
        context: RenderContext::default(),
    }
}

fn input_with_theme(source: &str, theme: &str) -> RenderInput {
    input_with_vendor_config(
        source,
        serde_json::json!({
            "plantuml_theme": theme,
        }),
    )
}

fn input_with_vendor_config(source: &str, vendor_config: serde_json::Value) -> RenderInput {
    RenderInput {
        kind: DiagramKind::PlantUml,
        source: source.to_string(),
        config: RenderConfig { vendor_config },
        policy: RenderPolicy::default(),
        context: RenderContext::default(),
    }
}
