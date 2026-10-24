use super::{skia_matrix, transformed_text_bounds};
use resvg::usvg::{Node, Options, Tree};
use skia_safe::Point as SkiaPoint;
use tiny_skia::Transform;

#[test]
fn skia_matrix_maps_axes_and_crop_origin() {
    let matrix = skia_matrix(
        Transform::from_row(2.0, 3.0, 4.0, 5.0, 10.0, 20.0),
        7.0,
        11.0,
    );
    let mut values = [0.0; 9];
    matrix.get_9(&mut values);
    assert_eq!(values, [2.0, 4.0, 3.0, 3.0, 5.0, 9.0, 0.0, 0.0, 1.0]);
    let point = tiny_skia::Point::from_xy(1.25, -0.75);
    let mut tiny_point = point;
    let transform = Transform::from_row(2.0, 3.0, 4.0, 5.0, 10.0, 20.0);
    transform.map_point(&mut tiny_point);
    let skia_point = matrix.map_point(SkiaPoint::new(point.x, point.y));
    assert!((skia_point.x - (tiny_point.x - 7.0)).abs() < f32::EPSILON);
    assert!((skia_point.y - (tiny_point.y - 11.0)).abs() < f32::EPSILON);
}

#[test]
fn rotated_crop_union_has_exact_shifted_endpoints() -> Result<(), String> {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><text id="run" x="8" y="20" font-family="Helvetica" font-size="16">A</text></svg>"#;
    let options = Options {
        fontdb: crate::markdown::svg_rasterize::font::html_font_db_for_text("Helvetica", "A"),
        ..Options::default()
    };
    let tree = Tree::from_str(source, &options).map_err(|error| error.to_string())?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("text node missing".to_owned());
    };
    let transform = Transform::from_row(0.96, 0.29, -0.29, 0.96, 4.0, 8.0);
    let base = transformed_text_bounds(text, transform, (0.0, 0.0)).ok_or("empty bounds")?;
    let shifted =
        transformed_text_bounds(text, transform, (-1.0, 1.0)).ok_or("empty shifted bounds")?;
    let expected_dx = 0.29;
    let expected_dy = 0.96;
    assert!((shifted.0 - (base.0 - expected_dx)).abs() < 0.0001);
    assert!((shifted.1 - (base.1 - expected_dy)).abs() < 0.0001);
    assert!((shifted.2 - (base.2 + expected_dx)).abs() < 0.0001);
    assert!((shifted.3 - (base.3 + expected_dy)).abs() < 0.0001);
    Ok(())
}
