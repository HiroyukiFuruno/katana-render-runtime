use super::{HtmlInteractiveSession, LayoutResult, SvgRasterizeOps};
use crate::renderer::backends::html_browser::{HtmlBrowserSource, HtmlBrowserViewport};

const INPUT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/html/aspects/input-state.html"
));
const RESERVATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/html/library-reservation-fidelity.html"
));
const BODY_MARKER: &str = "検索結果は通信環境";
const VIEWPORT_WIDTH: u32 = 620;
const VIEWPORT_HEIGHT: u32 = 900;

fn render_layout(html: &str) -> Result<(HtmlInteractiveSession, LayoutResult), String> {
    let source = HtmlBrowserSource::new(html, "https://example.invalid/font-geometry")
        .map_err(|error| error.to_string())?;
    let viewport = HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0)
        .map_err(|error| error.to_string())?;
    let mut session =
        HtmlInteractiveSession::start(source, viewport).map_err(|error| error.to_string())?;
    let layout = session
        .traced_layout(0)
        .map_err(|error| error.to_string())?;
    Ok((session, layout))
}

#[test]
fn linux_font_geometry_reports_text_input_width() -> Result<(), String> {
    let (session, layout) = render_layout(INPUT)?;
    let input = session
        .runtime
        .node_for_element_id("name")
        .ok_or("input missing")?;
    let hit = layout
        .hit_targets
        .iter()
        .find(|hit| hit.node_id == input.0)
        .ok_or("input layout missing")?;
    let family = "Arial, Liberation Sans, sans-serif";
    let width = SvgRasterizeOps::html_input_character_metrics(family, 13.3333, 400, false)
        .ok_or("input character metrics missing")?
        .width_for_size(20);
    let fonts = SvgRasterizeOps::html_text_font_runs("x0架空", family, 400, false);
    println!(
        "KRR_LINUX_FONT_GEOMETRY input width={} height={} content_width={width} fonts={fonts:?}",
        hit.width, hit.height
    );
    assert!(hit.width > width && hit.height > 0.0);
    Ok(())
}

#[test]
fn linux_font_geometry_reports_reservation_baseline() -> Result<(), String> {
    let (session, layout) = render_layout(RESERVATION)?;
    let target = session
        .runtime
        .node_for_element_id("column-p1")
        .ok_or("paragraph missing")?;
    let paragraph = layout
        .element_boxes
        .iter()
        .find(|item| item.node_id == target.0)
        .ok_or("paragraph layout missing")?;
    let (y, family) = first_line_baseline(&layout.svg)?;
    println!(
        "KRR_LINUX_FONT_GEOMETRY reservation top={} baseline={y} offset={} text_font={family:?}",
        paragraph.y,
        y - paragraph.y,
    );
    assert!(y > paragraph.y && y < paragraph.y + paragraph.height);
    Ok(())
}

fn first_line_baseline(svg: &str) -> Result<(f32, Option<String>), String> {
    let document = roxmltree::Document::parse(svg).map_err(|error| error.to_string())?;
    let line = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node
                    .descendants()
                    .filter_map(|child| child.text())
                    .any(|text| text.contains(BODY_MARKER))
        })
        .ok_or("first paragraph line missing")?;
    let y: f32 = line
        .attribute("y")
        .ok_or("text baseline missing")?
        .parse::<f32>()
        .map_err(|error| error.to_string())?;
    Ok((y, line.attribute("font-family").map(str::to_owned)))
}
