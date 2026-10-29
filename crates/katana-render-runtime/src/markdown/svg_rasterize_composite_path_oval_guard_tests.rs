use super::*;

const REJECTED_TRANSFORM_COUNT: usize = 5;

#[test]
fn source_oval_guard_accepts_reflection_and_nonuniform_scale() -> Result<(), String> {
    for transform in [
        Transform::identity(),
        Transform {
            sx: NONUNIFORM_SCALE_X,
            kx: ZERO,
            tx: ZERO,
            ky: ZERO,
            sy: NONUNIFORM_SCALE_Y,
            ty: ZERO,
        },
        Transform {
            sx: REFLECTION_SCALE_X,
            kx: ZERO,
            tx: REFLECTION_TRANSLATE_X,
            ky: ZERO,
            sy: ONE,
            ty: ZERO,
        },
    ] {
        assert!(uses_native_oval(OVAL_SVG, transform)?, "{transform:?}");
    }
    Ok(())
}

#[test]
fn source_oval_guard_rejects_edge_contact_skew_zero_and_unbounded_translation() -> Result<(), String>
{
    for transform in rejected_transforms() {
        assert!(!uses_native_oval(OVAL_SVG, transform)?, "{transform:?}");
    }
    assert_invalid_source_ellipses();
    Ok(())
}

fn rejected_transforms() -> [Transform; REJECTED_TRANSFORM_COUNT] {
    [
        Transform::from_translate(EDGE_TRANSLATE_X, ZERO),
        Transform::from_scale(ZERO, ONE),
        Transform {
            sx: ONE,
            kx: SKEW_X,
            tx: ZERO,
            ky: ZERO,
            sy: ONE,
            ty: ZERO,
        },
        Transform::from_translate(OUT_OF_BOUNDS_TRANSLATE_X, ZERO),
        Transform::from_scale(f32::MAX, f32::MAX),
    ]
}

fn assert_invalid_source_ellipses() {
    assert!(
        geometry::source_oval(
            (OVAL_CX, OVAL_CY, ZERO, OVAL_RY),
            skia_safe::PathFillType::Winding
        )
        .is_none()
    );
    assert!(
        geometry::source_oval(
            (f32::NAN, OVAL_CY, OVAL_RX, OVAL_RY),
            skia_safe::PathFillType::Winding
        )
        .is_none()
    );
}

#[test]
fn source_oval_guard_rejects_stroke_path_effects() -> Result<(), String> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><ellipse cx="16" cy="20" rx="8" ry="6" fill="#e02020" stroke="#2040e0" stroke-width="2" stroke-dasharray="4 2"/></svg>"##;
    assert!(!uses_native_oval(svg, Transform::identity())?);
    Ok(())
}

#[test]
fn oval_paint_order_preserves_stroke_and_fill_order() -> Result<(), String> {
    let fill_then_stroke = render_ordered_oval("fill stroke")?;
    let stroke_then_fill = render_ordered_oval("stroke fill")?;
    let first = fill_then_stroke
        .pixel(ORDER_SAMPLE.0, ORDER_SAMPLE.1)
        .ok_or("fill-stroke sample missing")?;
    let second = stroke_then_fill
        .pixel(ORDER_SAMPLE.0, ORDER_SAMPLE.1)
        .ok_or("stroke-fill sample missing")?;
    assert!(first.blue() > first.red());
    assert!(second.red() > second.blue());
    Ok(())
}

fn render_ordered_oval(order: &str) -> Result<Pixmap, String> {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"64\" height=\"64\"><ellipse cx=\"16\" cy=\"20\" rx=\"8\" ry=\"8\" fill=\"#e02020\" stroke=\"#2040e0\" stroke-width=\"6\" paint-order=\"{order}\"/></svg>"
    );
    let tree = parse(&svg)?;
    let node = first_path(&tree)?;
    let mut output = canvas()?;
    render_if_supported_with_tree(
        node,
        Transform::identity(),
        &mut output.as_mut(),
        Some(&tree),
    )?;
    Ok(output)
}
