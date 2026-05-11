use katana_canvas_forge::markdown::color_preset::DiagramColorPreset;
use katana_canvas_forge::markdown::drawio_renderer::DrawioRendererOps;
use katana_canvas_forge::markdown::{DiagramBlock, DiagramKind, DiagramResult};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

static ENV_LOCK: Mutex<()> = Mutex::new(());
const SIMPLE_DRAWIO_XML: &str = r#"<mxfile><diagram name="test"><mxGraphModel><root>
<mxCell id="0"/>
<mxCell id="1" parent="0"/>
<mxCell id="2" value="Box A" style="rounded=1;fillColor=#fff2cc;strokeColor=#d6b656;" vertex="1" parent="1">
    <mxGeometry x="80" y="80" width="120" height="60" as="geometry"/>
</mxCell>
<mxCell id="3" value="Box B" vertex="1" parent="1">
    <mxGeometry x="280" y="80" width="120" height="60" as="geometry"/>
</mxCell>
</root></mxGraphModel></diagram></mxfile>"#;
const COMPRESSED_DRAWIO_XML: &str = r#"<mxfile><diagram name="compressed">hVFBDsIgEHwN57ZgfEDR9OTJFxDZFBIoBKilv5fKqtGk8bAJMzsz2V0I4zYPQXh1cRIMYWfCeHAu1ZfNHIwhtNWSsBOhtC3V7PS6UgV6EWBK/+W0yu/CzFAZ7qwPECPIwvcuV0FMq0FBcPMkYfN3hPWL0gmuXty27lJ2KJxK1mAb4yEkyLsDPimcbwBnIYW1SNBwaKtj/YaLlkmhnyKnQI8KQ4/IiVjx+A5+HaPBa1TwuXfz8xkP</diagram></mxfile>"#;
const IMAGE_DRAWIO_XML: &str = r#"<mxGraphModel><root>
<mxCell id="0"/>
<mxCell id="1" parent="0"/>
<mxCell id="2" value="" style="shape=image;image=img/lib/ibm/miscellaneous/cognitive_services.svg;aspect=fixed;html=1;" vertex="1" parent="1">
    <mxGeometry x="40" y="40" width="80" height="80" as="geometry"/>
</mxCell>
</root></mxGraphModel>"#;
const OFFICIAL_STENCIL_DRAWIO_XML: &str = r#"<mxGraphModel><root>
<mxCell id="0"/>
<mxCell id="1" parent="0"/>
<mxCell id="2" value="" style="shape=mxgraph.basic.oval_callout;fillColor=#6c8ebf;strokeColor=#6c8ebf;html=1;" vertex="1" parent="1">
    <mxGeometry x="40" y="40" width="120" height="120" as="geometry"/>
</mxCell>
</root></mxGraphModel>"#;

#[test]
fn renders_key_drawio_cases_when_runtime_is_available() -> TestResult<()> {
    let _guard = env_guard()?;
    if DrawioRendererOps::find_drawio_js()?.is_none() {
        return Ok(());
    }

    let simple_svg = expect_drawio_svg(render_with_official_drawio_js(SIMPLE_DRAWIO_XML)?)?;
    let compressed_svg = expect_drawio_svg(render_with_official_drawio_js(COMPRESSED_DRAWIO_XML)?)?;
    let image_svg = expect_drawio_svg(render_with_official_drawio_js(IMAGE_DRAWIO_XML)?)?;
    let stencil_svg =
        expect_drawio_svg(render_with_official_drawio_js(OFFICIAL_STENCIL_DRAWIO_XML)?)?;

    assert!(simple_svg.contains("Box A"));
    assert!(compressed_svg.contains("Compressed Box"));
    assert!(image_svg.contains("<image"));
    assert!(stencil_svg.contains("<path"));
    Ok(())
}

#[test]
fn renders_official_aws4_fixture_when_runtime_is_available() -> TestResult<()> {
    let _guard = env_guard()?;
    if DrawioRendererOps::find_drawio_js()?.is_none() {
        return Ok(());
    }

    let source = include_str!("../../../tests/fixtures/drawio/official/templates/aws/aws_9.drawio");
    let svg = expect_drawio_svg(render_with_official_drawio_js(source)?)?;

    assert!(svg.contains("AWS Config"));
    assert!(svg.matches("<path").count() > 40);
    Ok(())
}

#[test]
fn returns_not_installed_without_drawio_js() -> TestResult<()> {
    let missing_path = missing_runtime_path("drawio-official");

    let result = DrawioRendererOps::render_drawio_with_runtime_path(
        &drawio_block(SIMPLE_DRAWIO_XML),
        &missing_path,
        DiagramColorPreset::current(),
    );

    assert!(matches!(result, DiagramResult::NotInstalled { .. }));
    Ok(())
}

fn render_with_official_drawio_js(source: &str) -> TestResult<DiagramResult> {
    let Some(drawio_js) = DrawioRendererOps::find_drawio_js()? else {
        return Err(test_error("Draw.io JavaScript must exist before rendering"));
    };
    Ok(DrawioRendererOps::render_drawio_with_runtime_path(
        &drawio_block(source),
        &drawio_js,
        DiagramColorPreset::current(),
    ))
}

fn missing_runtime_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("kcf-{name}-missing-{}.js", std::process::id()))
}

fn drawio_block(source: &str) -> DiagramBlock {
    DiagramBlock {
        kind: DiagramKind::DrawIo,
        source: source.to_string(),
    }
}

fn expect_drawio_svg(result: DiagramResult) -> TestResult<String> {
    match result {
        DiagramResult::Ok(svg) => Ok(svg),
        other => Err(test_error(format!("Expected Draw.io SVG, got {other:?}"))),
    }
}

fn env_guard() -> TestResult<MutexGuard<'static, ()>> {
    ENV_LOCK
        .lock()
        .map_err(|error| test_error(format!("environment lock is poisoned: {error}")))
}

fn test_error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other(message.into()))
}
