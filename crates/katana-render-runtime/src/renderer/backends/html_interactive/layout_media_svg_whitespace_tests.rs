use super::test_support::{TestResult, capture_frame_pixels};

const CAPTURE_WIDTH: u32 = 160;
const CAPTURE_HEIGHT: u32 = 48;
const CHANNEL_COUNT: usize = 4;
const RGB_CHANNEL_COUNT: usize = 3;
const ALPHA_INDEX: usize = 3;
const BLACK_CHANNEL_MAX: u8 = 64;
const TEXT_WITH_SPACES: &str = "DOM   shim";
const TEXT_WITH_ONE_SPACE: &str = "DOM shim";

#[test]
fn embedded_svg_whitespace_follows_child_context_in_the_real_frame() -> TestResult {
    let default_frame = capture_svg(&svg_markup(None, None, TEXT_WITH_SPACES))?;
    let single_frame = capture_svg(&svg_markup(None, None, TEXT_WITH_ONE_SPACE))?;
    let preserved_frame = capture_svg(&svg_markup(Some("preserve"), None, TEXT_WITH_SPACES))?;
    let child_default_frame = capture_svg(&svg_markup(
        Some("preserve"),
        Some("default"),
        TEXT_WITH_SPACES,
    ))?;
    let empty_frame = capture_svg(&svg_markup(None, None, ""))?;

    assert_real_ink(&default_frame, &empty_frame)?;
    assert!(
        default_frame == single_frame,
        "SVG default whitespace was not collapsed"
    );
    assert!(
        default_frame == child_default_frame,
        "child whitespace override differs"
    );
    assert!(
        default_frame != preserved_frame,
        "explicit whitespace preservation was lost"
    );
    assert_real_ink(&preserved_frame, &empty_frame)?;
    Ok(())
}

fn svg_markup(root_space: Option<&str>, child_space: Option<&str>, text: &str) -> String {
    let root_space =
        root_space.map_or_else(String::new, |value| format!(r#" xml:space="{value}""#));
    let child_space =
        child_space.map_or_else(String::new, |value| format!(r#" xml:space="{value}""#));
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="48"{root_space}><g{child_space}><text x="4" y="28" font-family="Helvetica" font-size="16">{text}</text></g></svg>"#
    )
}

fn capture_svg(svg: &str) -> TestResult<Vec<u8>> {
    let html = format!(
        "<!doctype html><html><head><style>html,body{{margin:0}}</style></head><body>{svg}</body></html>"
    );
    capture_frame_pixels(
        &html,
        "https://example.test/svg-whitespace.html",
        CAPTURE_WIDTH,
        CAPTURE_HEIGHT,
    )
}

fn assert_real_ink(pixels: &[u8], empty: &[u8]) -> TestResult {
    if pixels.len() != (CAPTURE_WIDTH * CAPTURE_HEIGHT * CHANNEL_COUNT as u32) as usize {
        return Err("unexpected frame dimensions".into());
    }
    if pixels == empty {
        return Err("whitespace regression frame is empty".into());
    }
    if pixels.chunks_exact(CHANNEL_COUNT).all(|pixel| {
        pixel[ALPHA_INDEX] == 0
            || pixel[..RGB_CHANNEL_COUNT]
                .iter()
                .any(|channel| *channel > BLACK_CHANNEL_MAX)
    }) {
        return Err("whitespace regression frame has no ink".into());
    }
    Ok(())
}
