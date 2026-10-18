use super::*;
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use resvg::usvg::fontdb::{Database, ID};
use rustybuzz::ttf_parser::Face;
use std::path::Path;

type TestResult<T = ()> = Result<T, String>;

const COLOR_EMOJI_FAMILY: &str = "Apple Color Emoji";
const COLOR_EMOJI_FONT_PATH: &str = "/System/Library/Fonts/Apple Color Emoji.ttc";
const REPRESENTATIVE_TEXT: &str = "0";
const MISSING_GLYPH: char = 'x';
const ITALIC: bool = false;
const NO_LETTER_SPACING: f32 = 0.0;
const ZERO_CSS_LENGTH: &str = "0";
const INPUT_BACKGROUND_HEX: &str = "#ff0000";
/* WHY: 入力レイアウトの比例文字幅係数を、公開APIの描画幅期待値にも使う。 */
const PROPORTIONAL_CHARACTER_WIDTH_FACTOR: f32 = 0.55;
const INPUT_FONT_SIZE: f32 = 16.0;
const INPUT_FONT_WEIGHT: u16 = 400;
const SMALL_INPUT_SIZE: usize = 7;
const LARGE_INPUT_SIZE: usize = 14;
const EXPECTED_WIDTH_TOLERANCE: f32 = 1.0;
const LINEAR_WIDTH_TOLERANCE: f32 = 3.0;
const INPUT_WIDTH_RATIO: f32 = 2.0;
const MIN_EXPECTED_WIDTH: f32 = 0.0;
const INPUT_VIEWPORT_WIDTH: u32 = 640;
const INPUT_VIEWPORT_HEIGHT: u32 = 80;
const DEVICE_SCALE_FACTOR: f32 = 1.0;
const PIXEL_BYTES: usize = 4;
const RGB_CHANNELS: usize = 3;
const RED_CHANNEL: usize = 0;
const GREEN_CHANNEL: usize = 1;
const BLUE_CHANNEL: usize = 2;
const ALPHA_CHANNEL: usize = RGB_CHANNELS;
const EMPTY_PIXEL_RUN: usize = 0;
const OPAQUE_ALPHA: u8 = 255;
const INPUT_BACKGROUND_RGB: [u8; RGB_CHANNELS] = [255, 0, 0];

#[test]
fn color_emoji_input_uses_representative_width_when_x_is_missing() -> TestResult {
    let database = apple_color_emoji_database()?;
    assert_all_faces_have_no_x(&database)?;
    assert_runtime_selects_color_emoji_without_x()?;
    assert_rendered_widths_follow_representative_width()
}

fn assert_runtime_selects_color_emoji_without_x() -> TestResult {
    assert!(
        SvgRasterizeOps::html_input_character_metrics(
            COLOR_EMOJI_FAMILY,
            INPUT_FONT_SIZE,
            INPUT_FONT_WEIGHT,
            ITALIC
        )
        .is_none()
    );
    let runs = SvgRasterizeOps::html_text_font_runs(
        REPRESENTATIVE_TEXT,
        COLOR_EMOJI_FAMILY,
        INPUT_FONT_WEIGHT,
        ITALIC,
    )
    .ok_or_else(|| "Apple Color Emoji did not resolve for representative glyph".to_string())?;
    assert!(runs.iter().any(|(family, _)| family == COLOR_EMOJI_FAMILY));
    Ok(())
}

fn assert_rendered_widths_follow_representative_width() -> TestResult {
    let expected_small = expected_input_width(SMALL_INPUT_SIZE)?;
    let expected_large = expected_input_width(LARGE_INPUT_SIZE)?;
    let actual_small = render_input_width(SMALL_INPUT_SIZE)?;
    let actual_large = render_input_width(LARGE_INPUT_SIZE)?;
    assert!((actual_small - expected_small).abs() <= EXPECTED_WIDTH_TOLERANCE);
    assert!((actual_large - expected_large).abs() <= EXPECTED_WIDTH_TOLERANCE);
    assert!(actual_small.is_finite() && actual_small > MIN_EXPECTED_WIDTH);
    assert!(actual_large.is_finite() && actual_large > actual_small);
    assert!((actual_large - actual_small * INPUT_WIDTH_RATIO).abs() <= LINEAR_WIDTH_TOLERANCE);
    Ok(())
}

fn expected_input_width(size: usize) -> TestResult<f32> {
    let representative = SvgRasterizeOps::measure_html_text(
        REPRESENTATIVE_TEXT,
        COLOR_EMOJI_FAMILY,
        INPUT_FONT_SIZE,
        INPUT_FONT_WEIGHT,
        ITALIC,
        NO_LETTER_SPACING,
        None,
    )
    .map_err(|error| format!("representative glyph measurement failed: {error:?}"))?
    .max(INPUT_FONT_SIZE * PROPORTIONAL_CHARACTER_WIDTH_FACTOR);
    Ok((representative * size as f32).ceil())
}

fn apple_color_emoji_database() -> TestResult<Database> {
    let path = Path::new(COLOR_EMOJI_FONT_PATH);
    assert!(path.is_file(), "Apple Color Emoji font file is unavailable");
    let mut database = Database::new();
    database
        .load_font_file(path)
        .map_err(|error| error.to_string())?;
    assert!(
        database.faces().any(|face| face
            .families
            .iter()
            .any(|(name, _)| name == COLOR_EMOJI_FAMILY)),
        "Apple Color Emoji faces are unavailable"
    );
    Ok(database)
}

fn assert_all_faces_have_no_x(database: &Database) -> TestResult {
    let faces = database
        .faces()
        .filter(|face| {
            face.families
                .iter()
                .any(|(name, _)| name == COLOR_EMOJI_FAMILY)
        })
        .map(|face| face.id)
        .collect::<Vec<ID>>();
    for face in faces {
        assert_face_has_no_x(database, face)?;
    }
    Ok(())
}

fn assert_face_has_no_x(database: &Database, face_id: ID) -> TestResult {
    let has_x = database
        .with_face_data(face_id, |data, index| {
            Face::parse(data, index).map(|face| face.glyph_index(MISSING_GLYPH).is_some())
        })
        .ok_or_else(|| "Apple Color Emoji face data is unavailable".to_string())?
        .map_err(|error| format!("Apple Color Emoji face parsing failed: {error:?}"))?;
    assert!(!has_x, "Apple Color Emoji unexpectedly contains glyph x");
    Ok(())
}

fn render_input_width(size: usize) -> TestResult<f32> {
    let html = format!(
        "<style>html,body{{margin:{ZERO_CSS_LENGTH};padding:{ZERO_CSS_LENGTH}}}input{{display:block}}</style><input size=\"{size}\" style=\"font-family:'{COLOR_EMOJI_FAMILY}';font-size:{INPUT_FONT_SIZE}px;padding:{ZERO_CSS_LENGTH};border:{ZERO_CSS_LENGTH};background:{INPUT_BACKGROUND_HEX}\">"
    );
    let source = HtmlBrowserSource::new(html, "https://example.test/input-width.html")
        .map_err(|error| error.to_string())?;
    let viewport = HtmlBrowserViewport::new(
        INPUT_VIEWPORT_WIDTH,
        INPUT_VIEWPORT_HEIGHT,
        DEVICE_SCALE_FACTOR,
    )
    .map_err(|error| error.to_string())?;
    let mut session =
        HtmlBrowserSession::new(source, viewport).map_err(|error| error.to_string())?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "initial input frame is missing".to_string())?;
    let width = red_input_width(frame);
    session.close().map_err(|error| error.to_string())?;
    width.ok_or_else(|| "rendered input background is missing".to_string())
}

fn red_input_width(frame: &HtmlBrowserFrame) -> Option<f32> {
    let row_width = frame.viewport.width as usize;
    let mut widest = EMPTY_PIXEL_RUN;
    for row in frame.pixels.chunks_exact(row_width * PIXEL_BYTES) {
        update_widest_red_run(row, &mut widest);
    }
    (widest > EMPTY_PIXEL_RUN).then_some(widest as f32)
}

fn update_widest_red_run(row: &[u8], widest: &mut usize) {
    let mut run = EMPTY_PIXEL_RUN;
    for pixel in row.chunks_exact(PIXEL_BYTES) {
        let pixel_rgb = [
            pixel[RED_CHANNEL],
            pixel[GREEN_CHANNEL],
            pixel[BLUE_CHANNEL],
        ];
        if pixel_rgb == INPUT_BACKGROUND_RGB && pixel[ALPHA_CHANNEL] == OPAQUE_ALPHA {
            run += 1;
            *widest = (*widest).max(run);
        } else {
            run = EMPTY_PIXEL_RUN;
        }
    }
}
