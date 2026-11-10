use super::{parse_with_languages, sample_path, usvg};

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

#[test]
fn overflowing_offscreen_transform_preserves_visible_sibling() -> Result<(), String> {
    let source = "<g transform='scale(1e20)'><path id='overflow' d='M1e19 0 L2e19 1' stroke='red'/></g><rect id='sample' width='4' height='4' fill='blue'/>";
    let control = "<rect id='sample' width='4' height='4' fill='blue'/>";
    let actual = parse_shape(source)?;
    let expected = parse_shape(control)?;
    assert!(actual.node_by_id("sample").is_some());
    assert!(actual.node_by_id("overflow").is_none());
    let mut actual_image = tiny_skia::Pixmap::new(8, 8).ok_or("actual pixmap missing")?;
    let mut expected_image = tiny_skia::Pixmap::new(8, 8).ok_or("expected pixmap missing")?;
    super::super::resvg::render(
        &actual,
        tiny_skia::Transform::identity(),
        &mut actual_image.as_mut(),
    );
    super::super::resvg::render(
        &expected,
        tiny_skia::Transform::identity(),
        &mut expected_image.as_mut(),
    );
    assert_eq!(actual_image.data(), expected_image.data());
    assert!(
        expected_image
            .pixels()
            .iter()
            .any(|pixel| pixel.alpha() > 0)
    );
    Ok(())
}
