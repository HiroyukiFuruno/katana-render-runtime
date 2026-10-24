use super::super::{layout::HtmlLayoutRenderer, style::CssStyle};
use crate::renderer::backends::html_browser::HtmlBrowserViewport;
use std::collections::HashMap;

const VIEWPORT_WIDTH: u32 = 320;
const VIEWPORT_HEIGHT: u32 = 240;
const TEXT_BASELINE: f32 = 24.0;

fn text_y_positions(svg: &str) -> Result<Vec<f32>, String> {
    svg.split("<text ")
        .skip(1)
        .map(|text| {
            text.split_once(" y=\"")
                .and_then(|(_, value)| value.split_once('"').map(|(y, _)| y))
                .ok_or_else(|| "text element has no y attribute".to_string())?
                .parse::<f32>()
                .map_err(|error| error.to_string())
        })
        .collect()
}

fn test_layout() -> (HtmlLayoutRenderer, CssStyle) {
    let viewport = HtmlBrowserViewport {
        width: VIEWPORT_WIDTH,
        height: VIEWPORT_HEIGHT,
        device_scale_factor: 1.0,
    };
    let renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
    let mut style = CssStyle::browser_default();
    style.text_baseline_offset = Some(0.0);
    (renderer, style)
}

fn paint_lines(renderer: &mut HtmlLayoutRenderer, lines: &[String], style: &CssStyle) {
    renderer.paint_text_lines(lines, 0.0, 100.0, TEXT_BASELINE, style);
}

fn assert_empty_line_spacing(svg: &str, line_height: f32) -> Result<(), String> {
    let positions = text_y_positions(svg)?;
    assert_eq!(positions.len(), 2);
    assert_eq!(positions[1], positions[0] + 2.0 * line_height);
    Ok(())
}

#[test]
fn empty_lines_skip_paint_but_keep_spacing_and_whitespace() -> Result<(), String> {
    let (mut renderer, style) = test_layout();
    let initial_svg = renderer.svg.clone();
    paint_lines(&mut renderer, &[String::new()], &style);
    assert_eq!(renderer.svg, initial_svg);
    paint_lines(
        &mut renderer,
        &["x".to_string(), String::new(), "x".to_string()],
        &style,
    );
    assert_empty_line_spacing(&renderer.svg, style.line_height)?;
    let before_empty = renderer.svg.clone();
    paint_lines(&mut renderer, &[String::new()], &style);
    assert_eq!(renderer.svg, before_empty);
    paint_lines(
        &mut renderer,
        &[
            " ".to_string(),
            "\u{00a0}".to_string(),
            "\u{200b}".to_string(),
        ],
        &style,
    );
    assert_eq!(renderer.svg.matches("<text").count(), 5);
    Ok(())
}
