use crate::{HtmlBrowserFrame, HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime};

const TABLE_WITH_EMPTY_FLEXIBLE_CELLS: &str = "<table style=\"width:120px;border-collapse:collapse;border-spacing:0\"><tr><td style=\"height:20px;padding:0;border:0;background:#ff0000\"></td><td style=\"height:20px;padding:0;border:0;background:#0000ff\"></td></tr></table>";
const TABLE_WITH_CENTERED_CONTENT: &str = "<table style=\"width:60px;border-collapse:collapse\"><tr><td style=\"height:40px;padding:0;border:0;background:#ff0000;vertical-align:middle\"><div style=\"width:10px;height:10px;background:#0000ff\"></div></td></tr></table>";
const TABLE_ORIGIN: &str = "https://example.test/empty-table.html";
const VIEWPORT_WIDTH: u32 = 160;
const VIEWPORT_HEIGHT: u32 = 80;
const RGB_CHANNELS: usize = 3;
const RGBA_CHANNELS: usize = 4;
const OPAQUE_ALPHA: u8 = 255;
const RED: [u8; RGB_CHANNELS] = [255, 0, 0];
const BLUE: [u8; RGB_CHANNELS] = [0, 0, 255];
const EXPECTED_CELL_WIDTH: usize = 60;
const EXPECTED_CELL_HEIGHT: usize = 20;
const EXPECTED_CELL_PIXELS: usize = EXPECTED_CELL_WIDTH * EXPECTED_CELL_HEIGHT;
const CENTERED_ROW_HEIGHT: usize = 40;
const CENTERED_CONTENT_SIZE: usize = 10;
const CENTERED_CONTENT_OFFSET: usize = (CENTERED_ROW_HEIGHT - CENTERED_CONTENT_SIZE) / 2;
const CONTENT_BOX_PAINT_HEIGHT: usize = 34;
const BORDER_BOX_PAINT_HEIGHT: usize = 20;

#[derive(Debug)]
struct ColorBand {
    left: usize,
    right: usize,
    top: usize,
    bottom: usize,
    pixels: usize,
}

impl ColorBand {
    fn at(x: usize, y: usize) -> Self {
        Self {
            left: x,
            right: x,
            top: y,
            bottom: y,
            pixels: 1,
        }
    }

    fn include(self, x: usize, y: usize) -> Self {
        Self {
            left: self.left.min(x),
            right: self.right.max(x),
            top: self.top.min(y),
            bottom: self.bottom.max(y),
            pixels: self.pixels + 1,
        }
    }
}

#[test]
fn empty_table_columns_render_as_equal_ordered_color_bands() -> Result<(), String> {
    assert_table_frame(TABLE_WITH_EMPTY_FLEXIBLE_CELLS, assert_ordered_equal_bands)
}

#[test]
fn table_cell_specified_height_preserves_middle_content_alignment() -> Result<(), String> {
    assert_table_frame(TABLE_WITH_CENTERED_CONTENT, assert_centered_content)
}

#[test]
fn table_cell_specified_height_respects_padding_and_collapsed_border() -> Result<(), String> {
    for (sizing, expected) in [
        ("content-box", CONTENT_BOX_PAINT_HEIGHT),
        ("border-box", BORDER_BOX_PAINT_HEIGHT),
    ] {
        let html = format!(
            "<table style=\"width:60px;border-collapse:collapse\"><tr><td style=\"height:20px;padding:5px;border:2px solid #ff0000;background:#ff0000;box-sizing:{sizing}\"></td></tr></table>"
        );
        assert_table_frame(&html, |frame| {
            assert_specified_cell_background(frame, expected)
        })?;
    }
    Ok(())
}

fn assert_specified_cell_background(
    frame: &HtmlBrowserFrame,
    expected: usize,
) -> Result<(), String> {
    let cell = color_band(frame, RED).ok_or_else(|| "table red cell missing".to_owned())?;
    if cell.right - cell.left + 1 != EXPECTED_CELL_WIDTH
        || cell.bottom - cell.top + 1 != expected
        || cell.pixels != EXPECTED_CELL_WIDTH * expected
    {
        return Err(format!(
            "specified cell must paint width60 height{expected}: {cell:?}"
        ));
    }
    Ok(())
}

fn assert_table_frame(
    html: &str,
    check: impl Fn(&HtmlBrowserFrame) -> Result<(), String>,
) -> Result<(), String> {
    let source = HtmlBrowserSource::new(html, TABLE_ORIGIN)
        .map_err(|error| format!("table source rejected: {error}"))?;
    let viewport = HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0)
        .map_err(|error| format!("table viewport rejected: {error}"))?;
    let mut session = HtmlRuntime
        .open(source, viewport)
        .map_err(|error| format!("table session open failed: {error}"))?;
    let bands = session
        .latest_frame()
        .ok_or_else(|| "table session returned no frame".to_owned())
        .and_then(check);
    let closed = session
        .close()
        .map_err(|error| format!("table session close failed: {error}"));
    match (bands, closed) {
        (Ok(()), result) | (result, Ok(())) => result,
        (Err(bands), Err(close)) => Err(format!("{bands}; {close}")),
    }
}

fn assert_centered_content(frame: &HtmlBrowserFrame) -> Result<(), String> {
    let row = color_band(frame, RED).ok_or_else(|| "table red row missing".to_owned())?;
    let content = color_band(frame, BLUE).ok_or_else(|| "table blue content missing".to_owned())?;
    let width = content.right - content.left + 1;
    let height = content.bottom - content.top + 1;
    if row.bottom - row.top + 1 != CENTERED_ROW_HEIGHT
        || (width, height) != (CENTERED_CONTENT_SIZE, CENTERED_CONTENT_SIZE)
        || content.pixels != CENTERED_CONTENT_SIZE * CENTERED_CONTENT_SIZE
        || content.top != row.top + CENTERED_CONTENT_OFFSET
    {
        return Err(format!(
            "table content must stay vertically centered: row={row:?}, content={content:?}"
        ));
    }
    Ok(())
}

fn assert_ordered_equal_bands(frame: &HtmlBrowserFrame) -> Result<(), String> {
    let red = color_band(frame, RED).ok_or_else(|| "table red band missing".to_owned())?;
    let blue = color_band(frame, BLUE).ok_or_else(|| "table blue band missing".to_owned())?;
    if red.right >= blue.left {
        return Err(format!(
            "red band must precede blue band: red={red:?}, blue={blue:?}"
        ));
    }
    assert_cell_geometry("red", &red)?;
    assert_cell_geometry("blue", &blue)?;
    if (red.top, red.bottom) != (blue.top, blue.bottom) {
        return Err(format!(
            "empty cells must share row bounds: red={red:?}, blue={blue:?}"
        ));
    }
    Ok(())
}

fn assert_cell_geometry(label: &str, band: &ColorBand) -> Result<(), String> {
    let width = band.right - band.left + 1;
    let height = band.bottom - band.top + 1;
    if (width, height, band.pixels)
        != (
            EXPECTED_CELL_WIDTH,
            EXPECTED_CELL_HEIGHT,
            EXPECTED_CELL_PIXELS,
        )
    {
        return Err(format!(
            "{label} cell must fill 60x20 opaque pixels: {band:?}"
        ));
    }
    Ok(())
}

fn color_band(frame: &HtmlBrowserFrame, color: [u8; RGB_CHANNELS]) -> Option<ColorBand> {
    let width = frame.viewport.width as usize;
    frame
        .pixels
        .as_chunks::<RGBA_CHANNELS>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, pixel)| pixel[0..RGB_CHANNELS] == color && pixel[RGB_CHANNELS] == OPAQUE_ALPHA)
        .map(|(index, _)| (index % width, index / width))
        .fold(None, |bounds: Option<ColorBand>, (x, y)| {
            Some(bounds.map_or_else(|| ColorBand::at(x, y), |band| band.include(x, y)))
        })
}
