use katana_render_runtime::{HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime};

const VIEWPORT_WIDTH: u32 = 180;
const VIEWPORT_HEIGHT: u32 = 180;

fn viewport() -> Result<HtmlBrowserViewport, Box<dyn std::error::Error>> {
    Ok(HtmlBrowserViewport::new(
        VIEWPORT_WIDTH,
        VIEWPORT_HEIGHT,
        1.0,
    )?)
}

fn open_alignment(
    alignment: &str,
) -> Result<katana_render_runtime::HtmlBrowserSession, Box<dyn std::error::Error>> {
    let html = format!(
        "<body style='margin:0'><table style='border-collapse:collapse;width:120px'><tr><td style='border:1px solid #246;vertical-align:{alignment}'><div style='width:24px;height:20px;background:#ff0000'></div></td><td><div style='height:100px'></div></td></tr></table>"
    );
    Ok(HtmlRuntime.open(
        HtmlBrowserSource::new(html, "https://example.test/table-vertical-align.html")?,
        viewport()?,
    )?)
}

fn open_group_alignment(
    group: &str,
    row: &str,
    cell: &str,
) -> Result<katana_render_runtime::HtmlBrowserSession, Box<dyn std::error::Error>> {
    let html = format!(
        "<body style='margin:0'><table style='border-collapse:collapse;width:120px'><tbody style='{group}'><tr style='{row}'><td style='border:1px solid #246;{cell}'><div style='width:24px;height:20px;background:#ff0000'></div></td><td><div style='height:100px'></div></td></tr></tbody></table>"
    );
    Ok(HtmlRuntime.open(
        HtmlBrowserSource::new(html, "https://example.test/table-vertical-align-group.html")?,
        viewport()?,
    )?)
}

fn colored_bounds(frame: &katana_render_runtime::HtmlBrowserFrame) -> Option<(u32, u32, u32, u32)> {
    let width = frame.viewport.width as usize;
    let height = frame.viewport.height as usize;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;
    for (index, pixel) in frame.pixels.chunks_exact(4).enumerate() {
        if pixel[..3] != [255, 0, 0] {
            continue;
        }
        let x = index % width;
        let y = index / width;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
        found = true;
    }
    found.then_some((min_x as u32, min_y as u32, max_x as u32, max_y as u32))
}

#[test]
fn public_table_vertical_alignments_place_colored_cell_content_by_actual_bounds()
-> Result<(), Box<dyn std::error::Error>> {
    let top = open_alignment("top")?;
    let middle = open_alignment("middle")?;
    let bottom = open_alignment("bottom")?;
    let top_bounds = colored_bounds(top.latest_frame().ok_or("top frame missing")?)
        .ok_or("top colored child missing")?;
    let middle_bounds = colored_bounds(middle.latest_frame().ok_or("middle frame missing")?)
        .ok_or("middle colored child missing")?;
    let bottom_bounds = colored_bounds(bottom.latest_frame().ok_or("bottom frame missing")?)
        .ok_or("bottom colored child missing")?;
    assert!(
        top_bounds.1 < middle_bounds.1,
        "top={top_bounds:?} middle={middle_bounds:?} bottom={bottom_bounds:?}"
    );
    assert!(
        middle_bounds.1 < bottom_bounds.1,
        "top={top_bounds:?} middle={middle_bounds:?} bottom={bottom_bounds:?}"
    );
    assert_eq!(top_bounds.1, 1);
    assert_eq!(middle_bounds.1, 41);
    assert_eq!(bottom_bounds.1, 80);
    assert_eq!(top_bounds.3 - top_bounds.1, 19);
    assert_eq!(middle_bounds.3 - middle_bounds.1, 19);
    assert_eq!(bottom_bounds.3 - bottom_bounds.1, 19);
    Ok(())
}

#[test]
fn public_table_vertical_alignment_cascades_group_row_and_cell_styles()
-> Result<(), Box<dyn std::error::Error>> {
    let group_top = open_group_alignment("vertical-align:top", "", "")?;
    let row_bottom = open_group_alignment("vertical-align:top", "vertical-align:bottom", "")?;
    let cell_middle = open_group_alignment("vertical-align:top", "", "vertical-align:middle")?;
    let top = colored_bounds(group_top.latest_frame().ok_or("group frame missing")?)
        .ok_or("group colored child missing")?;
    let bottom = colored_bounds(row_bottom.latest_frame().ok_or("row frame missing")?)
        .ok_or("row colored child missing")?;
    let middle = colored_bounds(cell_middle.latest_frame().ok_or("cell frame missing")?)
        .ok_or("cell colored child missing")?;
    assert_eq!(top.1, 1);
    assert_eq!(middle.1, 41);
    assert_eq!(bottom.1, 80);
    Ok(())
}
