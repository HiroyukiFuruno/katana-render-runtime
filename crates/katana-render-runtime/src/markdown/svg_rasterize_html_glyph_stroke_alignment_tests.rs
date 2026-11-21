use super::{HEIGHT, HtmlGlyphRasterizer, STROKE, WIDTH, fixture, text};
use crate::markdown::svg_rasterize::html_glyph::{
    Prepared, paint::PaintCaptureScope, surface::Crop,
};
use serde_json::Value;
use tiny_skia::{Pixmap, Point, Transform};

const ANCHORED_SCALE: &str = "text-anchor='end' transform='scale(0.9999877)'";
const TEXT: &str = "2038";
const STROKE_DRAW_COUNT: usize = 2;
const TRANSLATE_X: f32 = 70.0;

#[test]
fn stroke_preserves_plain_glyph_positions_and_transform() -> Result<(), String> {
    let plain = capture(ANCHORED_SCALE, 1)?;
    let stroked = capture(&format!("{ANCHORED_SCALE} {STROKE}"), STROKE_DRAW_COUNT)?;
    for row in &stroked {
        for key in [
            "glyphIds",
            "positions",
            "origin",
            "localToDeviceColumnMajor",
        ] {
            assert_eq!(plain[0][key], row[key], "glyph draw differs for {key}");
        }
    }
    Ok(())
}

fn capture(style: &str, expected_draws: usize) -> Result<Vec<Value>, String> {
    let (tree, _fixture) = fixture(style, TEXT)?;
    let text = text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(prepared) = rasterizer.prepare(text)? else {
        return Err("text was not prepared".to_owned());
    };
    let transform =
        Transform::from_translate(TRANSLATE_X, 0.0).pre_concat(text.flattened().transform());
    assert_within_canvas(text, transform);
    let crop = Crop {
        x: 0,
        y: 0,
        width: WIDTH,
        height: HEIGHT,
    };
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).ok_or("pixmap unavailable")?;
    let scope = PaintCaptureScope::enter();
    rasterizer.draw_prepared(prepared, transform, crop, &mut pixmap.as_mut())?;
    let snapshot = scope.snapshot();
    assert_eq!(snapshot.dropped, 0);
    assert_eq!(snapshot.events.len(), expected_draws);
    Ok(snapshot.events)
}

fn assert_within_canvas(text: &resvg::usvg::Text, transform: Transform) {
    let bounds = text.flattened().stroke_bounding_box();
    let mut corners = [
        Point::from_xy(bounds.x(), bounds.y()),
        Point::from_xy(bounds.right(), bounds.bottom()),
    ];
    transform.map_points(&mut corners);
    assert!(corners[0].x >= 0.0 && corners[0].y >= 0.0);
    assert!(corners[1].x <= WIDTH as f32 && corners[1].y <= HEIGHT as f32);
}
