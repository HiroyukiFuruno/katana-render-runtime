use super::{Mode, NORMAL_WIDTH, parse_source, render};
use std::path::Path;
use tiny_skia::Pixmap;

const ORIGINAL_RADIUS: &str = "radius=\"10 0\"";
const FILTER_ATTRIBUTE: &str = " filter=\"url(#filter1)\"";
const INPUT_SIZE: f32 = 200.0;
const CHANNELS: usize = 4;

pub(super) fn assert_one_axis_contract(input: &Path) -> Result<(), String> {
    let source = std::fs::read_to_string(input).map_err(|error| error.to_string())?;
    let original = render_source(input, &source)?;
    let subpixel = replace_once(&source, ORIGINAL_RADIUS, "radius=\"10 0.1\"")?;
    let no_filter = replace_once(&source, FILTER_ATTRIBUTE, "")?;
    let whole_axis = replace_once(&source, ORIGINAL_RADIUS, "radius=\"10 1\"")?;
    let subpixel = render_source(input, &subpixel)?;
    let no_filter = render_source(input, &no_filter)?;
    let whole_axis = render_source(input, &whole_axis)?;
    assert_eq!(different_pixels(&original, &subpixel)?, 0, "subpixel axis");
    assert!(
        different_pixels(&original, &no_filter)? > 0,
        "filter must affect output"
    );
    assert!(
        different_pixels(&original, &whole_axis)? > 0,
        "whole-pixel axis must affect output"
    );
    Ok(())
}

fn replace_once(source: &str, from: &str, to: &str) -> Result<String, String> {
    if source.matches(from).count() != 1 {
        return Err(format!("expected exactly one fixture token: {from}"));
    }
    Ok(source.replacen(from, to, 1))
}

fn render_source(input: &Path, source: &str) -> Result<Pixmap, String> {
    let tree = parse_source(input, source.as_bytes())?;
    assert_eq!(
        (tree.size().width(), tree.size().height()),
        (INPUT_SIZE, INPUT_SIZE)
    );
    let pixmap = render(&tree, Mode::Normal)?;
    assert_eq!(
        (pixmap.width(), pixmap.height()),
        (NORMAL_WIDTH, NORMAL_WIDTH)
    );
    Ok(pixmap)
}

fn different_pixels(left: &Pixmap, right: &Pixmap) -> Result<usize, String> {
    if left.width() != right.width() || left.height() != right.height() {
        return Err("morphology comparison dimensions differ".to_owned());
    }
    Ok(left
        .data()
        .chunks_exact(CHANNELS)
        .zip(right.data().chunks_exact(CHANNELS))
        .filter(|(a, b)| a != b)
        .count())
}

#[test]
fn comparison_helpers_reject_ambiguous_input_and_size_mismatch() -> Result<(), String> {
    let one = Pixmap::new(1, 1).ok_or("one-pixel pixmap unavailable")?;
    let two = Pixmap::new(2, 1).ok_or("two-pixel pixmap unavailable")?;
    for (label, result) in [
        ("missing token", replace_once("none", "x", "y").map(|_| ())),
        ("duplicate token", replace_once("xx", "x", "y").map(|_| ())),
        (
            "different dimensions",
            different_pixels(&one, &two).map(|_| ()),
        ),
    ] {
        assert!(result.is_err(), "{label} must be rejected");
    }
    Ok(())
}
