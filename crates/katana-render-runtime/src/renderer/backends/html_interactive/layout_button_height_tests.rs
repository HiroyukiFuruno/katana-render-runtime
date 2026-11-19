use crate::renderer::backends::html_browser::{
    HtmlBrowserInput, HtmlBrowserSource, HtmlBrowserViewport,
};
use crate::renderer::backends::html_interactive::HtmlInteractiveSession;
use crate::renderer::backends::html_interactive::tests::test_error;

const BOTTOM_PADDING_TEST_OFFSET: f32 = 2.5;
const RGBA_CHANNELS: usize = 4;
const BUTTON_BACKGROUND_RGBA: [u8; RGBA_CHANNELS] = [54, 154, 188, 255];
const HTML: &str = r#"<style>
    button { display:block; font-family:Arial; font-size:14px; line-height:28px;
        padding:3px 4px 5px; border:2px solid; }
    #fixed { height:50px; } #minimum { min-height:45px; }
</style><button id="auto" style="background-color:#369abc">保存</button><button id="fixed">固定</button>
<button id="minimum">最小</button><script>
    document.getElementById('auto').addEventListener('click', function () {
        this.setAttribute('id', 'activated');
    });
</script>"#;

#[test]
fn auto_button_uses_label_line_box_and_receives_click_at_bottom_padding() -> Result<(), String> {
    let source =
        HtmlBrowserSource::new(HTML, "https://example.test/buttons").map_err(test_error)?;
    let viewport = HtmlBrowserViewport::new(320, 240, 1.0).map_err(test_error)?;
    let mut session = HtmlInteractiveSession::start(source, viewport).map_err(test_error)?;
    let (x, y) = assert_button_box(&mut session, "auto", 40.0)?;
    assert_button_box(&mut session, "fixed", 50.0)?;
    assert_button_box(&mut session, "minimum", 45.0)?;
    assert_painted_button(&mut session, x, y)?;
    for input in [
        HtmlBrowserInput::PointerDown { x, y, button: 0 },
        HtmlBrowserInput::PointerUp { x, y, button: 0 },
    ] {
        session.dispatch_input(input).map_err(test_error)?;
    }
    assert!(session.runtime.node_for_element_id("activated").is_some());
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
) -> Result<(f32, f32), String> {
    let node_id = session.runtime.node_for_element_id(id).ok_or(id)?.0;
    let target = session
        .hit_targets
        .iter()
        .find(|target| target.node_id == node_id)
        .ok_or("missing button target")?;
    assert_eq!(target.height, height, "{id}");
    let y = target.y + height - BOTTOM_PADDING_TEST_OFFSET;
    Ok((target.x + target.width / 2.0, y))
}
