use super::{CssStyle, append_border};

const BOX_WIDTH: f32 = 100.0;
const BOX_HEIGHT: f32 = 40.0;
const CORNER_INTERIOR_PIXEL: (u32, u32) = (94, 3);
const TOP_CORNER_PIXEL: (u32, u32) = (96, 2);
const RIGHT_CORNER_PIXEL: (u32, u32) = (98, 4);

#[test]
fn zero_width_uniform_border_emits_no_outline() {
    let style = CssStyle {
        border: Some("red".to_string()),
        border_width: 0.0,
        ..CssStyle::browser_default()
    };
    let mut svg = String::new();
    append_border(&mut svg, 0.0, 0.0, BOX_WIDTH, BOX_HEIGHT, &style);
    assert!(svg.is_empty());
}

#[test]
fn rounded_border_with_absent_edge_and_consumed_inner_radius_stays_hollow()
-> Result<(), Box<dyn std::error::Error>> {
    let style = CssStyle::from_attributes(
        &[("style".to_string(),
            "border-top:0px solid red;border-right:4px solid blue;border-bottom:4px solid green;border-left:4px solid orange;border-radius:2px".to_string())],
        &CssStyle::browser_default(),
    );
    let mut svg = String::new();
    append_border(&mut svg, 0.0, 0.0, BOX_WIDTH, BOX_HEIGHT, &style);
    assert_eq!(svg.matches("<path").count(), 3);
    let pixels = border_pixels(&svg)?;
    assert_eq!(
        pixels.pixel(50, 0).ok_or("top pixel must exist")?.alpha(),
        0
    );
    assert_eq!(
        pixels
            .pixel(50, 20)
            .ok_or("center pixel must exist")?
            .alpha(),
        0
    );
    assert!(pixels.pixel(99, 20).ok_or("right pixel must exist")?.blue() > 0);
    Ok(())
}

#[test]
fn equal_explicit_border_edges_keep_one_continuous_rounded_outline()
-> Result<(), Box<dyn std::error::Error>> {
    let style = CssStyle::from_attributes(
        &[("style".to_string(),
            "border-top:1px solid #e2e8f0;border-right:1px solid #e2e8f0;border-bottom:1px solid #e2e8f0;border-left:1px solid #e2e8f0;border-radius:8px".to_string())],
        &CssStyle::browser_default(),
    );
    let mut svg = String::new();
    append_border(&mut svg, 0.0, 0.0, BOX_WIDTH, BOX_HEIGHT, &style);
    assert_eq!(svg.matches("<rect").count(), 1);
    assert!(!svg.contains("<path"));
    assert!(svg.contains(r##"rx="7.5" ry="7.5" fill="none" stroke="#e2e8f0" stroke-width="1""##));
    assert_corner_interior_unpainted(&svg)?;
    Ok(())
}

fn assert_corner_interior_unpainted(border: &str) -> Result<(), Box<dyn std::error::Error>> {
    let pixels = border_pixels(border)?;
    let (x, y) = CORNER_INTERIOR_PIXEL;
    assert_eq!(
        pixels.pixel(x, y).ok_or("corner pixel must exist")?.alpha(),
        0
    );
    Ok(())
}

fn border_pixels(border: &str) -> Result<resvg::tiny_skia::Pixmap, Box<dyn std::error::Error>> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{BOX_WIDTH}" height="{BOX_HEIGHT}">{border}</svg>"#
    );
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())?;
    let mut pixels = resvg::tiny_skia::Pixmap::new(BOX_WIDTH as u32, BOX_HEIGHT as u32)
        .ok_or("border pixel buffer must fit")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixels.as_mut(),
    );
    Ok(pixels)
}

#[test]
fn differently_colored_rounded_edges_meet_without_painting_the_inner_corner()
-> Result<(), Box<dyn std::error::Error>> {
    let style = CssStyle::from_attributes(
        &[("style".to_string(),
            "border-top:1px solid red;border-right:1px solid blue;border-bottom:1px solid green;border-left:4px solid orange;border-radius:8px".to_string())],
        &CssStyle::browser_default(),
    );
    let mut svg = String::new();
    append_border(&mut svg, 0.0, 0.0, BOX_WIDTH, BOX_HEIGHT, &style);
    assert_corner_interior_unpainted(&svg)?;
    let pixels = border_pixels(&svg)?;
    let top = pixels
        .pixel(TOP_CORNER_PIXEL.0, TOP_CORNER_PIXEL.1)
        .ok_or("top corner must exist")?;
    let right = pixels
        .pixel(RIGHT_CORNER_PIXEL.0, RIGHT_CORNER_PIXEL.1)
        .ok_or("right corner must exist")?;
    assert!(top.red() > top.blue());
    assert!(right.blue() > right.red());
    Ok(())
}

#[test]
fn absent_uniform_border_does_not_emit_an_svg_outline() {
    let mut svg = String::new();
    super::uniform::append_uniform_border(
        &mut svg,
        0.0,
        0.0,
        BOX_WIDTH,
        BOX_HEIGHT,
        &CssStyle::browser_default(),
    );
    assert!(svg.is_empty());
}
