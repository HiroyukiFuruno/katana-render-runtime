use super::super::style::{CssBorderStyle, CssBoxSizing};
use super::{CssStyle, input_geometry, is_checkbox};

#[test]
fn auto_height_input_includes_line_height_padding_and_border() {
    let mut style = CssStyle::browser_default();
    style.line_height = 33.6;
    style.padding_top = 16.0;
    style.padding_bottom = 16.0;
    style.border_width = 1.0;
    style.border_style = CssBorderStyle::Solid;

    let geometry = input_geometry(0.0, 0.0, 300.0, &style);

    assert!((geometry.height - 67.6).abs() < 0.01);
}

#[test]
fn border_box_explicit_input_height_is_not_expanded() {
    let mut style = CssStyle::browser_default();
    style.box_sizing = CssBoxSizing::BorderBox;
    style.height = Some(40.0);
    style.padding_top = 8.0;
    style.padding_bottom = 8.0;

    assert_eq!(input_geometry(0.0, 0.0, 300.0, &style).height, 40.0);
}

#[test]
fn checkbox_type_attribute_is_detected() {
    let attrs = vec![("type".to_string(), "checkbox".to_string())];

    assert!(is_checkbox(&attrs));
}

#[cfg(target_os = "linux")]
#[test]
fn linux_text_input_uses_the_resolved_arial_stack_for_intrinsic_width() -> Result<(), String> {
    use crate::renderer::backends::html_browser::{HtmlBrowserSource, HtmlBrowserViewport};
    use crate::renderer::backends::html_interactive::HtmlInteractiveSession;
    use crate::renderer::backends::html_interactive::tests::test_error;
    use crate::renderer::backends::html_interactive::types::HitTargetKind;

    const VIEWPORT_WIDTH: u32 = 620;
    const VIEWPORT_HEIGHT: u32 = 900;
    let html = r#"<style>.reference {font-family:Arial, 'Liberation Sans', sans-serif}</style>
        <label>名称<input id="default"></label><input id="reference" class="reference">
        <input id="short" size="10"><input id="short-reference" size="10" class="reference">"#;
    let source = HtmlBrowserSource::new(html, "https://example.test/inputs").map_err(test_error)?;
    let viewport =
        HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0).map_err(test_error)?;
    let mut session = HtmlInteractiveSession::start(source, viewport).map_err(test_error)?;
    let mut width = |id: &str| -> Result<f32, String> {
        let node = session.runtime.node_for_element_id(id).ok_or(id)?;
        session
            .hit_targets
            .iter()
            .find(|target| target.node_id == node.0 && matches!(target.kind, HitTargetKind::Input))
            .map(|target| target.width)
            .ok_or_else(|| format!("input target missing: {id}"))
    };
    assert_eq!(width("default")?, width("reference")?);
    assert_eq!(width("short")?, width("short-reference")?);
    assert!(width("default")? > width("short")?);
    Ok(())
}
