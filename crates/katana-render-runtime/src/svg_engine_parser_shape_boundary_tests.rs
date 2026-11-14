use super::{CHANNEL_COUNT, center_pixel, parse_with_languages, sample_path, usvg};
use rstest::rstest;

#[rstest]
#[case::inside_clip(4, [0, 0, 255, 255])]
#[case::outside_clip(6, [0, 0, 0, 0])]
fn clip_path_ignores_text_and_non_graphic_children(
    #[case] clip_x: u32,
    #[case] expected_pixel: [u8; CHANNEL_COUNT],
) -> Result<(), String> {
    let source = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <defs><clipPath id="mask">ignored text<!-- ignored comment -->
      <title>clip description</title><rect x="{clip_x}" y="4" width="2" height="4"/></clipPath></defs>
      <rect width="8" height="8" fill="#0000ff" clip-path="url(#mask)"/></svg>"##
    );
    assert_eq!(center_pixel(&source, &[])?, expected_pixel);
    Ok(())
}

fn parse_shape(shape: &str) -> Result<usvg::Tree, String> {
    let source =
        format!("<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'>{shape}</svg>");
    parse_with_languages(&source, &[])
}

#[test]
fn ellipse_negative_radius_and_path_parse_edges_preserve_only_valid_geometry() -> Result<(), String>
{
    let circle = parse_shape("<ellipse id='sample' rx='2' ry='2' fill='red'/>")?;
    for shape in [
        "<ellipse id='sample' rx='-2' ry='2' fill='red'/>",
        "<ellipse id='sample' rx='2' ry='-2' fill='red'/>",
    ] {
        let actual = parse_shape(shape)?;
        assert!(
            sample_path(&actual)?
                .data()
                .segments()
                .eq(sample_path(&circle)?.data().segments()),
            "{shape}"
        );
    }

    let invalid_ellipse = parse_shape("<ellipse id='sample' rx='-2' ry='-3' fill='red'/>")?;
    assert!(invalid_ellipse.node_by_id("sample").is_none());

    let valid_path = parse_shape("<path id='sample' d='M1 1 L7 1' fill='red'/>")?;
    let partial_path = parse_shape("<path id='sample' d='M1 1 L7 1 L' fill='red'/>")?;
    assert!(
        sample_path(&partial_path)?
            .data()
            .segments()
            .eq(sample_path(&valid_path)?.data().segments())
    );
    Ok(())
}

#[rstest::rstest]
#[case::zero(0, false)]
#[case::normal(4, true)]
fn valid_rx_does_not_hide_invalid_ry(
    #[case] ry: i32,
    #[case] expected: bool,
) -> Result<(), String> {
    let normal = parse_shape("<ellipse id='sample' rx='4' ry='4' fill='red'/>")?;
    let shape = format!("<ellipse id='sample' rx='4' ry='{ry}' fill='red'/>");
    let actual = parse_shape(&shape)?;
    assert_eq!(actual.node_by_id("sample").is_some(), expected, "{shape}");
    if expected {
        assert!(
            sample_path(&actual)?
                .data()
                .segments()
                .eq(sample_path(&normal)?.data().segments()),
            "{shape}"
        );
    }
    Ok(())
}

#[test]
fn polyline_and_polygon_without_points_are_skipped() -> Result<(), String> {
    for shape in [
        "<polyline id='sample' fill='red'/>",
        "<polygon id='sample' fill='red'/>",
    ] {
        let tree = parse_shape(shape)?;
        assert!(tree.node_by_id("sample").is_none(), "{shape}");
    }
    Ok(())
}

#[rstest]
#[case::move_only("<path id='ignored' d='M1 1' stroke='red'/>")]
#[case::missing_data("<path id='ignored' stroke='red'/>")]
fn incomplete_path_does_not_affect_a_visible_sibling(#[case] path: &str) -> Result<(), String> {
    let control = "<rect id='sample' width='4' height='4' fill='blue'/>";
    let source = format!("{path}{control}");
    let actual = parse_shape(&source)?;
    let expected = parse_shape(control)?;
    assert!(actual.node_by_id("ignored").is_none());
    assert!(actual.node_by_id("sample").is_some());
    assert_same_visible_shape_render(&actual, &expected)
}

#[test]
fn overflowing_offscreen_transform_preserves_visible_sibling() -> Result<(), String> {
    let source = "<g transform='scale(1e20)'><path id='overflow' d='M1e19 0 L2e19 1' stroke='red'/></g><rect id='sample' width='4' height='4' fill='blue'/>";
    let control = "<rect id='sample' width='4' height='4' fill='blue'/>";
    let actual = parse_shape(source)?;
    let expected = parse_shape(control)?;
    assert!(actual.node_by_id("sample").is_some());
    assert!(actual.node_by_id("overflow").is_none());
    assert_same_visible_shape_render(&actual, &expected)
}

#[test]
fn overflowing_rect_edges_preserve_visible_sibling() -> Result<(), String> {
    const X: f32 = 3e38;
    const WIDTH: f32 = 1e38;
    let control = "<rect id='sample' width='4' height='4' fill='blue'/>";
    let source = format!(
        "<rect id='square' x='{X:e}' width='{WIDTH:e}' height='4' fill='red'/><rect id='round' x='{X:e}' width='{WIDTH:e}' height='4' rx='1' fill='red'/>{control}"
    );
    assert!(X.is_finite() && WIDTH.is_finite() && (X + WIDTH).is_infinite());
    let actual = parse_shape(&source)?;
    let expected = parse_shape(control)?;
    assert!(actual.node_by_id("square").is_none());
    assert!(actual.node_by_id("round").is_none());
    assert!(actual.node_by_id("sample").is_some());
    assert_same_visible_shape_render(&actual, &expected)
}

#[test]
fn offscreen_isolated_group_with_large_bounds_preserves_visible_sibling() -> Result<(), String> {
    let source = "<g opacity='0.5'><rect id='offscreen' x='2147483392' width='512' height='4' fill='red'/></g><rect id='sample' width='4' height='4' fill='blue'/>";
    let control = "<rect id='sample' width='4' height='4' fill='blue'/>";
    let actual = parse_shape(source)?;
    let expected = parse_shape(control)?;
    assert!(actual.node_by_id("offscreen").is_some());
    assert_same_visible_shape_render(&actual, &expected)
}

fn assert_same_visible_shape_render(
    actual: &usvg::Tree,
    expected: &usvg::Tree,
) -> Result<(), String> {
    let actual_image = render_shape(actual)?;
    let expected_image = render_shape(expected)?;
    assert_eq!(actual_image.data(), expected_image.data());
    assert!(
        expected_image
            .pixels()
            .iter()
            .any(|pixel| pixel.alpha() > 0)
    );
    Ok(())
}

fn render_shape(tree: &usvg::Tree) -> Result<tiny_skia::Pixmap, String> {
    let size = tree.size().to_int_size();
    let mut image =
        tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("shape pixmap missing")?;
    super::super::resvg::render(tree, tiny_skia::Transform::identity(), &mut image.as_mut());
    Ok(image)
}
