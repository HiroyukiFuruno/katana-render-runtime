use super::{
    args::PreviewArgs, capture::PreviewCaptureWriter, paths::AppResult, report::PreviewCapture,
};
use katana_render_runtime::{
    HtmlBrowserInput, HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime,
};

const ACCORDION_X: f32 = 250.0;
const ACCORDION_Y: f32 = 214.0;
const ACTION_X: f32 = 175.0;
const ACTION_Y: f32 = 274.0;
const FORM_X: f32 = 460.0;
const FORM_Y: f32 = 274.0;
const CLICK_INPUT_COUNT: usize = 3;
const LINK_PROBE_X: f32 = 24.0;
const LINK_PROBE_Y: f32 = 24.0;

#[derive(Clone, Copy)]
struct ClickCapture {
    interaction: &'static str,
    x: f32,
    y: f32,
    label: &'static str,
    suffix: &'static str,
}

const ACCORDION_CAPTURE: ClickCapture = ClickCapture {
    interaction: "accordion click",
    x: ACCORDION_X,
    y: ACCORDION_Y,
    label: "Accordion opened by click",
    suffix: "accordion",
};
const BUTTON_CAPTURE: ClickCapture = ClickCapture {
    interaction: "action button click",
    x: ACTION_X,
    y: ACTION_Y,
    label: "Button click updated DOM",
    suffix: "button",
};
const TEXT_CAPTURE: ClickCapture = ClickCapture {
    interaction: "text input",
    x: FORM_X,
    y: FORM_Y,
    label: "Text input delivered",
    suffix: "typed",
};

pub(crate) struct PreviewInteraction;

impl PreviewInteraction {
    pub(crate) fn run_interaction_scenario(
        args: &PreviewArgs,
        session: &mut HtmlBrowserSession,
        captures: &mut Vec<PreviewCapture>,
    ) -> AppResult<()> {
        click_and_capture(args, session, captures, ACCORDION_CAPTURE)?;
        click_and_capture(args, session, captures, BUTTON_CAPTURE)?;
        type_and_capture(args, session, captures, TEXT_CAPTURE)
    }

    pub(crate) fn run_link_navigation_probe(
        origin: &str,
        viewport: HtmlBrowserViewport,
    ) -> AppResult<String> {
        eprintln!("interaction: link navigation probe");
        let source =
            HtmlBrowserSource::new(link_probe_html(), origin).map_err(|error| error.to_string())?;
        let mut session = HtmlRuntime
            .open(source, viewport)
            .map_err(|error| error.to_string())?;
        click(&mut session, LINK_PROBE_X, LINK_PROBE_Y)?;
        let navigation = session
            .take_navigation()
            .map(|navigation| navigation.url.as_str().to_string())
            .ok_or_else(|| "link navigation probe did not emit a navigation event".to_string())?;
        if let Err(error) = session.close() {
            eprintln!("warning: failed to close link probe cleanly: {error}");
        }
        Ok(navigation)
    }
}

fn click_and_capture(
    args: &PreviewArgs,
    session: &mut HtmlBrowserSession,
    captures: &mut Vec<PreviewCapture>,
    step: ClickCapture,
) -> AppResult<()> {
    eprintln!("interaction: {}", step.interaction);
    let previous = frame_generation(session, step.interaction)?;
    click(session, step.x, step.y)?;
    assert_frame_updated(session, previous, step.interaction)?;
    PreviewCaptureWriter::capture_latest(
        captures,
        step.label,
        &args.output_for(step.suffix),
        session,
    )
}

fn type_and_capture(
    args: &PreviewArgs,
    session: &mut HtmlBrowserSession,
    captures: &mut Vec<PreviewCapture>,
    step: ClickCapture,
) -> AppResult<()> {
    eprintln!("interaction: {}", step.interaction);
    let previous = frame_generation(session, step.interaction)?;
    click(session, step.x, step.y)?;
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: "ok".to_string(),
        })
        .map_err(|error| error.to_string())?;
    assert_frame_updated(session, previous, step.interaction)?;
    PreviewCaptureWriter::capture_latest(
        captures,
        step.label,
        &args.output_for(step.suffix),
        session,
    )
}

fn link_probe_html() -> &'static str {
    r#"<!doctype html><style>html,body{margin:0}a{display:block;width:160px;height:48px}</style><a href="linked-page.html">Open linked page</a>"#
}

fn click(session: &mut HtmlBrowserSession, x: f32, y: f32) -> AppResult<()> {
    for input in click_inputs(x, y) {
        session
            .dispatch_input(input)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn click_inputs(x: f32, y: f32) -> [HtmlBrowserInput; CLICK_INPUT_COUNT] {
    [
        HtmlBrowserInput::PointerMove { x, y },
        HtmlBrowserInput::PointerDown { x, y, button: 0 },
        HtmlBrowserInput::PointerUp { x, y, button: 0 },
    ]
}

fn frame_generation(session: &HtmlBrowserSession, label: &str) -> AppResult<u64> {
    session
        .latest_frame()
        .map(|frame| frame.generation)
        .ok_or_else(|| format!("browser session did not produce frame for {label}"))
}

fn assert_frame_updated(
    session: &HtmlBrowserSession,
    previous_generation: u64,
    interaction: &str,
) -> AppResult<()> {
    let generation = frame_generation(session, interaction)?;
    (generation > previous_generation)
        .then_some(())
        .ok_or_else(|| format!("{interaction} did not produce a repainted frame"))
}
