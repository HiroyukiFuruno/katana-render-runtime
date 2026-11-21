use super::{CssStyle, ElementRenderContext, button_height, button_style};
use crate::renderer::backends::html_browser::{
    HtmlBrowserInput, HtmlBrowserSource, HtmlBrowserViewport,
};
use crate::renderer::backends::html_interactive::HtmlInteractiveSession;
use crate::renderer::backends::html_interactive::tests::test_error;

const BOTTOM_PADDING_TEST_OFFSET: f32 = 2.5;
const OUTSIDE_BUTTON_OFFSET: f32 = 4.0;
const BUTTON_CENTER_DIVISOR: f32 = 2.0;
const RGBA_CHANNELS: usize = 4;
const BUTTON_BACKGROUND_RGBA: [u8; RGBA_CHANNELS] = [54, 154, 188, 255];
const HTML: &str = r#"<style>
    button { display:block; font-family:Arial; font-size:14px; line-height:28px;
        border:2px solid; }
    #auto, #fixed, #minimum { padding:3px 4px 5px; }
    #fixed { height:50px; } #minimum { min-height:45px; }
</style><button id="auto" style="background-color:#369abc">保存</button><button id="fixed">固定</button>
<button id="minimum">最小</button><button id="default-padding">既定</button><script>
    document.getElementById('auto').addEventListener('click', function () {
        this.setAttribute('id', 'activated');
    });
</script>"#;

#[test]
fn button_height_supplies_vertical_padding_when_the_computed_style_has_none() {
    let computed = CssStyle::browser_default();
    let mut style = button_style(&computed);
    assert!(!style.explicit_padding);
    let element = ElementRenderContext {
        node_id: 1,
        tag: "button",
        attributes: &[],
        children: &[],
    };
    let height = button_height(element, &computed, &mut style);
    assert_eq!((style.padding_top, style.padding_bottom), (1.0, 1.0));
    assert_eq!(height, style.line_height + 6.0);
}

#[test]
fn auto_button_uses_label_line_box_and_receives_click_at_bottom_padding() -> Result<(), String> {
    let source =
        HtmlBrowserSource::new(HTML, "https://example.test/buttons").map_err(test_error)?;
    let viewport = HtmlBrowserViewport::new(320, 240, 1.0).map_err(test_error)?;
    let mut session = HtmlInteractiveSession::start(source, viewport).map_err(test_error)?;
    let (x, y, right) = assert_button_box(&mut session, "auto", 40.0)?;
    assert_button_box(&mut session, "fixed", 50.0)?;
    assert_button_box(&mut session, "minimum", 45.0)?;
    assert_button_box(&mut session, "default-padding", 34.0)?;
    assert_painted_button(&mut session, x, y)?;
    click(&mut session, right + OUTSIDE_BUTTON_OFFSET, y)?;
    assert!(session.runtime.node_for_element_id("auto").is_some());
    click(&mut session, x, y)?;
    assert!(session.runtime.node_for_element_id("activated").is_some());
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn linux_default_button_height_uses_the_same_font_as_its_label() -> Result<(), String> {
    use crate::markdown::svg_rasterize::SvgRasterizeOps;

    let html = concat!(
        "<style>button { display:block }</style>",
        "<button id=\"latin\">OK</button><button id=\"japanese\">保存</button>",
    );
    let source =
        HtmlBrowserSource::new(html, "https://example.test/buttons").map_err(test_error)?;
    let viewport = HtmlBrowserViewport::new(320, 240, 1.0).map_err(test_error)?;
    let mut session = HtmlInteractiveSession::start(source, viewport).map_err(test_error)?;
    for (id, label) in [("latin", "OK"), ("japanese", "保存")] {
        let metrics = SvgRasterizeOps::html_line_metrics(
            label,
            "Arial, Liberation Sans, sans-serif",
            13.3333,
            400,
            false,
        )
        .ok_or("missing default control font metrics")?;
        assert_button_box(&mut session, id, metrics.0 + metrics.1 + 6.0)?;
    }
    Ok(())
}

fn click(session: &mut HtmlInteractiveSession, x: f32, y: f32) -> Result<(), String> {
    for input in [
        HtmlBrowserInput::PointerDown { x, y, button: 0 },
        HtmlBrowserInput::PointerUp { x, y, button: 0 },
    ] {
        session.dispatch_input(input).map_err(test_error)?;
    }
    Ok(())
}

fn assert_painted_button(
    session: &mut HtmlInteractiveSession,
    x: f32,
    y: f32,
) -> Result<(), String> {
    let frame = session.latest_frame().ok_or("missing button frame")?;
    let offset = (y as usize * frame.viewport.width as usize + x as usize) * RGBA_CHANNELS;
    let observed = frame.pixels.get(offset..offset + RGBA_CHANNELS);
    if observed != Some(BUTTON_BACKGROUND_RGBA.as_slice()) {
        let observed = observed.map(<[u8]>::to_vec);
        let layout = session.layout().map_err(test_error)?;
        return Err(format!(
            "button fill at ({x}, {y}): {observed:?}, SVG: {}",
            layout.svg
        ));
    }
    Ok(())
}

fn assert_button_box(
    session: &mut HtmlInteractiveSession,
    id: &str,
    height: f32,
) -> Result<(f32, f32, f32), String> {
    let node_id = session.runtime.node_for_element_id(id).ok_or(id)?.0;
    let mut targets = session
        .hit_targets
        .iter()
        .filter(|target| target.node_id == node_id);
    let target = targets.next().ok_or("missing button target")?;
    assert!(targets.next().is_none(), "{id} has multiple targets");
    let bounds = session
        .element_boxes
        .iter()
        .find(|bounds| bounds.node_id == node_id)
        .ok_or("missing button bounds")?;
    assert_eq!(
        (bounds.x, bounds.y, bounds.width, bounds.height),
        (target.x, target.y, target.width, target.height)
    );
    assert_eq!(target.height, height, "{id}");
    let y = target.y + height - BOTTOM_PADDING_TEST_OFFSET;
    Ok((
        target.x + target.width / BUTTON_CENTER_DIVISOR,
        y,
        target.x + target.width,
    ))
}
