use katana_render_runtime::{
    HtmlArtifactError, HtmlArtifactProvenance, HtmlArtifactStore, HtmlBrowserInput,
    HtmlBrowserSource, HtmlBrowserViewport, HtmlFrameArtifact, HtmlRuntime,
};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const PUBLIC_LAYOUT_HTML: &str = r#"<!doctype html>
<html>
<head>
  <style>
    body { margin: 8px; font: 16px Arial, sans-serif; }
    .flex { display: flex; gap: 8px; }
    .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 4px; }
    .cell { border: 1px solid #246; padding: 4px; background: #def; }
    table { border-collapse: collapse; margin-top: 8px; }
    td { border: 1px solid #642; padding: 3px; }
    input { width: 120px; }
    .spacer { height: 1100px; background: linear-gradient(#fff, #eef); }
  </style>
</head>
<body>
  <a id="next" href="/linked.html">Open linked document</a>
  <div class="flex"><div class="cell">Flex A</div><div class="cell">Flex B</div></div>
  <div class="grid"><div class="cell">Grid A</div><div class="cell">Grid B</div></div>
  <table><tr><td>Row one</td><td><input aria-label="value"></td></tr></table>
  <div class="spacer"></div>
</body>
</html>"#;

fn source() -> Result<HtmlBrowserSource, Box<dyn std::error::Error>> {
    Ok(HtmlBrowserSource::new(
        PUBLIC_LAYOUT_HTML,
        "https://example.test/public-layout.html",
    )?)
}

fn viewport() -> Result<HtmlBrowserViewport, Box<dyn std::error::Error>> {
    Ok(HtmlBrowserViewport::new(240, 160, 1.0)?)
}

fn contains_rgb(frame: &katana_render_runtime::HtmlBrowserFrame, rgb: [u8; 3]) -> bool {
    frame.pixels.chunks_exact(4).any(|pixel| pixel[..3] == rgb)
}

fn full_frame_similarity(left: &[u8], right: &[u8]) -> f64 {
    assert_eq!(left.len(), right.len());
    assert!(!left.is_empty());
    let (absolute, squared) = left.iter().zip(right).fold((0.0, 0.0), |(a, q), (l, r)| {
        let difference = f64::from(*l) - f64::from(*r);
        (a + difference.abs(), q + difference * difference)
    });
    let count = left.len() as f64;
    let mae = absolute / count / 255.0;
    let rmse = (squared / count).sqrt() / 255.0;
    (1.0 - mae).min(1.0 - rmse)
}

fn whitespace_table_html(white_space: &str, cell: &str) -> String {
    format!(
        "<body style='margin:0'><table style='border-collapse:collapse;width:180px'><tr><td style='border:1px solid #246;padding:3px;white-space:{white_space}'>{cell}</td></tr></table>"
    )
}

fn assert_table_whitespace_parity(white_space: &str) -> Result<(), Box<dyn std::error::Error>> {
    let baseline = HtmlRuntime.open(
        HtmlBrowserSource::new(
            whitespace_table_html(white_space, "alpha"),
            "https://example.test/table-whitespace.html",
        )?,
        viewport()?,
    )?;
    let padded = HtmlRuntime.open(
        HtmlBrowserSource::new(
            whitespace_table_html(white_space, "\n alpha \n"),
            "https://example.test/table-whitespace.html",
        )?,
        viewport()?,
    )?;
    let baseline_frame = baseline
        .latest_frame()
        .ok_or("baseline table frame missing")?;
    let padded_frame = padded
        .latest_frame()
        .ok_or("whitespace table frame missing")?;
    assert_eq!(baseline_frame.content_height, padded_frame.content_height);
    assert!(
        baseline_frame.pixels == padded_frame.pixels,
        "table whitespace changed pixels for {white_space}"
    );
    Ok(())
}

#[test]
fn public_table_whitespace_collapses_consistently_for_normal_and_nowrap()
-> Result<(), Box<dyn std::error::Error>> {
    for white_space in ["normal", "nowrap"] {
        assert_table_whitespace_parity(white_space)?;
    }
    Ok(())
}

fn open_table_cell(
    cell: &str,
) -> Result<katana_render_runtime::HtmlBrowserSession, Box<dyn std::error::Error>> {
    let html = format!(
        "<body style='margin:0'><table style='border-collapse:collapse;width:180px'><tr><td style='border:1px solid #246;padding:3px'>{cell}</td></tr></table><div style='height:200px'></div>"
    );
    Ok(HtmlRuntime.open(
        HtmlBrowserSource::new(html, "https://example.test/table-cell.html")?,
        viewport()?,
    )?)
}

#[test]
fn public_table_cells_preserve_nbsp_and_br_painting() -> Result<(), Box<dyn std::error::Error>> {
    let nbsp = open_table_cell("A&nbsp;B")?;
    let plain = open_table_cell("AB")?;
    let forced_break = open_table_cell("A<br>B")?;
    let plain_mixed = open_table_cell("AB<br>C")?;
    let mixed = open_table_cell("A<span>B</span><br>C")?;
    let larger = open_table_cell("A<span style='font-size:32px'>B</span><br>C")?;
    let nbsp_frame = nbsp.latest_frame().ok_or("NBSP frame missing")?;
    let plain_frame = plain.latest_frame().ok_or("plain frame missing")?;
    let break_frame = forced_break.latest_frame().ok_or("break frame missing")?;
    let plain_mixed_frame = plain_mixed
        .latest_frame()
        .ok_or("plain mixed frame missing")?;
    let mixed_frame = mixed.latest_frame().ok_or("mixed frame missing")?;
    let larger_frame = larger.latest_frame().ok_or("larger frame missing")?;
    assert!(
        nbsp_frame.pixels != plain_frame.pixels,
        "NBSP must retain painted spacing"
    );
    assert!(break_frame.content_height > plain_frame.content_height);
    assert!(
        break_frame.pixels != plain_frame.pixels,
        "forced break must change table painting"
    );
    assert_mixed_table_geometry(plain_mixed_frame, mixed_frame, larger_frame, break_frame);
    assert!(
        mixed_frame.pixels != plain_frame.pixels,
        "mixed forced break must retain its text"
    );
    Ok(())
}

fn assert_mixed_table_geometry(
    plain: &katana_render_runtime::HtmlBrowserFrame,
    mixed: &katana_render_runtime::HtmlBrowserFrame,
    larger: &katana_render_runtime::HtmlBrowserFrame,
    forced_break: &katana_render_runtime::HtmlBrowserFrame,
) {
    assert_eq!(plain.content_height, forced_break.content_height);
    assert_eq!(mixed.content_height, plain.content_height);
    assert!(full_frame_similarity(&mixed.pixels, &plain.pixels) >= 0.99);
    assert!(larger.content_height > plain.content_height);
}

#[test]
fn public_table_cells_neutral_nested_inline_preserves_line_geometry()
-> Result<(), Box<dyn std::error::Error>> {
    let plain = open_table_cell("AB<br>C")?;
    let nested = open_table_cell("A<span><span>B</span></span><br>C")?;
    let plain_frame = plain.latest_frame().ok_or("plain frame missing")?;
    let nested_frame = nested.latest_frame().ok_or("nested frame missing")?;
    let larger = open_table_cell("A<span><span style='font-size:32px'>B</span></span><br>C")?;
    let larger_frame = larger.latest_frame().ok_or("nested larger frame missing")?;
    assert!(larger_frame.content_height > plain_frame.content_height);
    assert_eq!(nested_frame.content_height, plain_frame.content_height);
    assert!(
        full_frame_similarity(&nested_frame.pixels, &plain_frame.pixels) >= 0.99,
        "neutral nested inline full-frame score below 99"
    );
    Ok(())
}

#[test]
fn public_neutral_inline_keeps_its_declared_text_color() -> Result<(), Box<dyn std::error::Error>> {
    let session = open_table_cell("A<span style='color:#ff0000'>B</span><br>C")?;
    let frame = session
        .latest_frame()
        .ok_or("colored inline frame missing")?;
    assert!(contains_rgb(frame, [255, 0, 0]));
    Ok(())
}

#[test]
fn public_in_flow_input_height_affects_line_box_but_absolute_input_does_not()
-> Result<(), Box<dyn std::error::Error>> {
    let in_flow = HtmlRuntime.open(
        HtmlBrowserSource::new(
            "<body style='margin:0'><div>label <input style='height:40px'></div><div style='height:300px'></div>",
            "https://example.test/input-line-box.html",
        )?,
        viewport()?,
    )?;
    let out_of_flow = HtmlRuntime.open(
        HtmlBrowserSource::new(
            "<body style='margin:0'><div>label <input style='height:40px;position:absolute'></div><div style='height:300px'></div>",
            "https://example.test/input-line-box.html",
        )?,
        viewport()?,
    )?;
    let in_flow_frame = in_flow
        .latest_frame()
        .ok_or("in-flow input frame missing")?;
    let out_of_flow_frame = out_of_flow
        .latest_frame()
        .ok_or("out-of-flow input frame missing")?;
    assert!(in_flow_frame.content_height > out_of_flow_frame.content_height);
    Ok(())
}

#[test]
fn invalid_border_styles_preserve_the_publicly_rendered_valid_border()
-> Result<(), Box<dyn std::error::Error>> {
    let baseline = "<body style='margin:0'><div style='width:120px;height:40px;border:3px solid #246;background:#def'>border</div>";
    let invalid = baseline.replace(
        "background:#def",
        "background:#def;border-style:invented;border-left-style:invented",
    );
    let valid_session = HtmlRuntime.open(
        HtmlBrowserSource::new(baseline, "https://example.test/border.html")?,
        viewport()?,
    )?;
    let invalid_session = HtmlRuntime.open(
        HtmlBrowserSource::new(invalid, "https://example.test/border.html")?,
        viewport()?,
    )?;
    let valid_frame = valid_session
        .latest_frame()
        .ok_or("valid border frame missing")?;
    let invalid_frame = invalid_session
        .latest_frame()
        .ok_or("invalid declaration frame missing")?;
    assert!(contains_rgb(valid_frame, [0x22, 0x44, 0x66]));
    assert!(contains_rgb(valid_frame, [0xdd, 0xee, 0xff]));
    assert_eq!(valid_frame.pixels, invalid_frame.pixels);
    Ok(())
}

fn assert_initial_layout(
    session: &katana_render_runtime::HtmlBrowserSession,
) -> Result<(), Box<dyn std::error::Error>> {
    let initial = session
        .latest_frame()
        .ok_or("public session must expose an initial frame")?;
    assert_eq!(initial.viewport, viewport()?);
    assert_eq!(initial.pixels.len(), 240 * 160 * 4);
    assert!(contains_rgb(initial, [0xdd, 0xee, 0xff]));
    assert!(contains_rgb(initial, [0x22, 0x44, 0x66]));
    assert!(initial.content_height > initial.viewport.height as f32);
    assert_eq!(initial.scroll_y, 0.0);
    Ok(())
}

fn assert_link_navigation(
    session: &mut katana_render_runtime::HtmlBrowserSession,
) -> Result<(), Box<dyn std::error::Error>> {
    session.dispatch_input(HtmlBrowserInput::PointerDown {
        x: 12.0,
        y: 12.0,
        button: 0,
    })?;
    session.dispatch_input(HtmlBrowserInput::PointerUp {
        x: 12.0,
        y: 12.0,
        button: 0,
    })?;
    let navigation = session
        .take_navigation()
        .ok_or("clicking the visible link must emit navigation")?;
    assert_eq!(navigation.url.as_str(), "https://example.test/linked.html");
    Ok(())
}

#[test]
fn public_runtime_renders_layout_scrolls_and_navigates() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = HtmlRuntime.open(source()?, viewport()?)?;
    assert_initial_layout(&session)?;
    assert_link_navigation(&mut session)?;
    let mut session = HtmlRuntime.open(source()?, viewport()?)?;
    session.dispatch_input(HtmlBrowserInput::Scroll {
        delta_x: 0.0,
        delta_y: 10_000.0,
    })?;
    let scrolled = session
        .latest_frame()
        .ok_or("scrolling must retain a frame")?;
    let max_scroll = (scrolled.content_height - scrolled.viewport.height as f32).max(0.0);
    assert!(scrolled.scroll_y > 0.0);
    assert!((scrolled.scroll_y - max_scroll).abs() < 0.01);
    assert!(contains_rgb(scrolled, [0xee, 0xee, 0xff]));
    Ok(())
}

#[test]
fn public_frame_artifact_round_trip_restores_scroll_and_rejects_other_source()
-> Result<(), Box<dyn std::error::Error>> {
    let source = source()?;
    let viewport = viewport()?;
    let mut session = HtmlRuntime.open(source.clone(), viewport)?;
    session.dispatch_input(HtmlBrowserInput::Scroll {
        delta_x: 0.0,
        delta_y: 10_000.0,
    })?;
    let frame = session
        .latest_frame()
        .ok_or("scrolled session must expose a frame")?
        .clone();
    assert!(frame.scroll_y > 0.0);
    let provenance = artifact_provenance(&source, viewport)?;
    let artifact = HtmlFrameArtifact::new(provenance.clone(), frame.clone())?;

    let store = artifact_store()?;
    store.save("scrolled", &artifact)?;
    let loaded = store
        .load("scrolled", &provenance)?
        .ok_or("saved public frame must be loadable")?;
    assert_eq!(loaded.frame, frame);

    assert_artifact_resume(&loaded, &source, &provenance, &frame)?;
    assert!(store.clear("scrolled")?);
    assert!(store.load("scrolled", &provenance)?.is_none());
    Ok(())
}

#[test]
fn public_frame_artifact_rejects_invalid_viewport_before_resume_open()
-> Result<(), Box<dyn std::error::Error>> {
    let source = source()?;
    let viewport = viewport()?;
    let session = HtmlRuntime.open(source.clone(), viewport)?;
    let frame = session
        .latest_frame()
        .ok_or("public session must expose an initial frame")?
        .clone();
    let provenance = artifact_provenance(&source, viewport)?;

    for invalid_viewport in [
        HtmlBrowserViewport {
            width: 0,
            ..viewport
        },
        HtmlBrowserViewport {
            device_scale_factor: 0.0,
            ..viewport
        },
    ] {
        let mut invalid = HtmlFrameArtifact::new(provenance.clone(), frame.clone())?;
        invalid.provenance.viewport = invalid_viewport;
        assert!(matches!(
            invalid.validate(),
            Err(HtmlArtifactError::InvalidProvenance(_))
        ));
        assert!(matches!(
            invalid.resume_session(source.clone(), viewport, &provenance),
            Err(HtmlArtifactError::InvalidProvenance(_))
        ));
    }
    Ok(())
}

fn assert_artifact_resume(
    loaded: &HtmlFrameArtifact,
    source: &HtmlBrowserSource,
    provenance: &HtmlArtifactProvenance,
    frame: &katana_render_runtime::HtmlBrowserFrame,
) -> Result<(), Box<dyn std::error::Error>> {
    let resumed = loaded.resume_session(source.clone(), frame.viewport, provenance)?;
    let resumed_frame = resumed
        .latest_frame()
        .ok_or("resumed session must expose a frame")?;
    assert!((resumed_frame.scroll_y - frame.scroll_y).abs() < 0.01);

    let changed_source = HtmlBrowserSource::new(
        format!("{PUBLIC_LAYOUT_HTML}<!-- changed -->"),
        source.origin.as_str(),
    )?;
    assert!(matches!(
        loaded.resume_session(changed_source, frame.viewport, provenance),
        Err(HtmlArtifactError::ProvenanceMismatch)
    ));
    Ok(())
}

fn artifact_provenance(
    source: &HtmlBrowserSource,
    viewport: HtmlBrowserViewport,
) -> Result<HtmlArtifactProvenance, Box<dyn std::error::Error>> {
    Ok(HtmlArtifactProvenance::for_source(
        source,
        viewport,
        "1111111111111111111111111111111111111111111111111111111111111111",
        "2222222222222222222222222222222222222222222222222222222222222222",
        "3333333333333333333333333333333333333333333333333333333333333333",
    )?)
}

fn artifact_store() -> Result<HtmlArtifactStore, Box<dyn std::error::Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("krr-public-layout-artifact-{unique}"));
    Ok(HtmlArtifactStore::new(&root, 16 * 1024 * 1024, true)?)
}
