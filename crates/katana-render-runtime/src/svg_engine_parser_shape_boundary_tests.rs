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
