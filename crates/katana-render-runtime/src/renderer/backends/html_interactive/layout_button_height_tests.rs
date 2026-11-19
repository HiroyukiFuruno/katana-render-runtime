use crate::renderer::backends::html_browser::{
    HtmlBrowserInput, HtmlBrowserSource, HtmlBrowserViewport,
};
use crate::renderer::backends::html_interactive::HtmlInteractiveSession;
use crate::renderer::backends::html_interactive::tests::test_error;

const HTML: &str = r#"<style>
    button { display:block; font:14px/28px Arial; padding:3px 4px 5px; border:2px solid; }
    #auto { background:#369abc; }
    #fixed { height:50px; } #minimum { min-height:45px; }
</style><button id="auto">保存</button><button id="fixed">固定</button>
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
    let (x, y) = assert_button_box(&session, "auto", 40.0)?;
    assert_button_box(&session, "fixed", 50.0)?;
    assert_button_box(&session, "minimum", 45.0)?;
    let frame = session.latest_frame().ok_or("missing button frame")?;
    let offset = (y as usize * frame.viewport.width as usize + x as usize) * 4;
    assert_eq!(
        frame.pixels.get(offset..offset + 4),
        Some(&[54, 154, 188, 255][..])
    );
    for input in [
        HtmlBrowserInput::PointerDown { x, y, button: 0 },
        HtmlBrowserInput::PointerUp { x, y, button: 0 },
    ] {
        session.dispatch_input(input).map_err(test_error)?;
    }
    assert!(session.runtime.node_for_element_id("activated").is_some());
    Ok(())
}

fn assert_button_box(
    session: &HtmlInteractiveSession,
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
    Ok((target.x + target.width / 2.0, target.y + height - 2.5))
}
