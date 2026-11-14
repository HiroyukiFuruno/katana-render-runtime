use super::{
    Arc, BBOX_HEIGHT, BBOX_WIDTH, BBOX_X, BBOX_Y, Cache, Paint, Point, Rect, Transform, Units,
    assert_authored_radial_values, assert_source_gradient, assert_transform_points,
    authored_gradient, cache, radial_gradient,
};

const ZERO_BBOX_WIDTH: f32 = 0.0;

fn assert_shared_bbox_conversion(
    original: &Arc<super::RadialGradient>,
    converted: &Arc<super::RadialGradient>,
) -> Result<(), String> {
    if Arc::ptr_eq(original, converted)
        || converted.base.units != Units::UserSpaceOnUse
        || converted.id() == super::GRADIENT_ID
    {
        return Err("shared gradient Arc or units contract changed".to_owned());
    }
    assert_source_gradient(original, Units::ObjectBoundingBox)?;
    assert_authored_radial_values(converted)?;
    assert_transform_points(
        converted.transform(),
        [
            Point::from_xy(BBOX_X, BBOX_Y),
            Point::from_xy(BBOX_X + BBOX_WIDTH, BBOX_Y),
            Point::from_xy(BBOX_X, BBOX_Y + BBOX_HEIGHT),
        ],
    )
}

fn process_bbox_paint(paint: &mut Paint, bbox: Rect, cache: &mut Cache) -> bool {
    super::process_paint(
        paint,
        false,
        Transform::identity(),
        None,
        Transform::identity(),
        bbox,
        cache,
    )
}

#[test]
fn shared_radial_gradient_is_cloned_into_bbox_user_coordinates() -> Result<(), String> {
    let source = authored_gradient(Units::ObjectBoundingBox)?;
    let original = Arc::clone(&source);
    let mut paint = Paint::RadialGradient(source);
    let mut cache = cache();
    let bbox = Rect::from_xywh(BBOX_X, BBOX_Y, BBOX_WIDTH, BBOX_HEIGHT)
        .ok_or_else(|| "test bbox is invalid".to_owned())?;

    if !process_bbox_paint(&mut paint, bbox, &mut cache) {
        return Err("non-zero object bbox paint was rejected".to_owned());
    }
    let converted = radial_gradient(&paint)?;
    assert_shared_bbox_conversion(&original, converted)
}

#[test]
fn zero_width_radial_bbox_is_rejected_without_mutating_the_paint() -> Result<(), String> {
    let source = authored_gradient(Units::ObjectBoundingBox)?;
    let original = Arc::clone(&source);
    let mut paint = Paint::RadialGradient(source);
    let before = paint.clone();
    let mut cache = cache();
    let bbox = Rect::from_xywh(BBOX_X, BBOX_Y, ZERO_BBOX_WIDTH, BBOX_HEIGHT)
        .ok_or_else(|| "zero-width test bbox is invalid".to_owned())?;
    assert!(!process_bbox_paint(&mut paint, bbox, &mut cache));
    assert_eq!(paint, before);
    assert!(Arc::ptr_eq(&original, radial_gradient(&paint)?));
    assert_source_gradient(&original, Units::ObjectBoundingBox)
}
