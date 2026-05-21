use katana_diagram_renderer::{
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
    assert_eq!(PLANTUML_JAR_VERSION, "1.2026.4");
    assert_eq!(PLANTUML_JAR_CHECKSUM.len(), 64);
    assert!(PLANTUML_DOWNLOAD_URL.contains("plantuml-lgpl"));
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
    let jar_path = RuntimePathResolver::resolve(DiagramKind::PlantUml, None)?;
    if !jar_path.exists() {
        return Ok(());
    }
    let renderer = PlantUmlRenderer::with_runtime_path(jar_path);
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
    let jar_path = RuntimePathResolver::resolve(DiagramKind::PlantUml, None)?;
    if !jar_path.exists() {
        return Ok(());
    }
    let renderer = PlantUmlRenderer::with_runtime_path(jar_path);
    for fixture in representative_fixtures() {
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

#[test]
fn plantuml_official_fixtures_render_when_local_runtime_is_available()
-> Result<(), Box<dyn std::error::Error>> {
    let jar_path = RuntimePathResolver::resolve(DiagramKind::PlantUml, None)?;
    if !jar_path.exists() {
        return Ok(());
    }
    let renderer = PlantUmlRenderer::with_runtime_path(jar_path);
    for fixture in OFFICIAL_FIXTURES {
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

#[test]
fn plantuml_dark_mode_uses_official_color_mapper_when_local_runtime_is_available()
-> Result<(), Box<dyn std::error::Error>> {
    let jar_path = RuntimePathResolver::resolve(DiagramKind::PlantUml, None)?;
    if !jar_path.exists() {
        return Ok(());
    }
    let renderer = PlantUmlRenderer::with_runtime_path(jar_path);
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
    let jar_path = RuntimePathResolver::resolve(DiagramKind::PlantUml, None)?;
    if !jar_path.exists() {
        return Ok(());
    }
    let renderer = PlantUmlRenderer::with_runtime_path(jar_path);
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
    let jar_path = RuntimePathResolver::resolve(DiagramKind::PlantUml, None)?;
    if !jar_path.exists() {
        return Ok(());
    }
    let renderer = PlantUmlRenderer::with_runtime_path(jar_path);
    let result = renderer.render(&input("@startuml\nthis is not valid ???\n@enduml"));

    assert!(matches!(
        result,
        Err(RenderError::Runtime(message)) if message.contains("PlantUML render failed")
    ));
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
    RenderInput {
        kind: DiagramKind::PlantUml,
        source: source.to_string(),
        config: RenderConfig {
            vendor_config: serde_json::json!({
                "plantuml_theme": theme,
            }),
        },
        policy: RenderPolicy::default(),
        context: RenderContext::default(),
    }
}
