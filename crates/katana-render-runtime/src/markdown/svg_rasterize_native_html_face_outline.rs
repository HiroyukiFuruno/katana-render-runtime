use std::ffi::c_void;

use objc2_core_foundation::{CGAffineTransform, CGPoint};
use objc2_core_graphics::{CGPath, CGPathElement, CGPathElementType};
use objc2_core_text::CTFont;
use tiny_skia_path::{Path, PathBuilder};

pub(super) fn outline(font: &CTFont, glyph_id: u16, units_per_em: u16) -> Option<Path> {
    let scale = outline_scale(font, units_per_em)?;
    let matrix = CGAffineTransform {
        a: scale,
        b: 0.0,
        c: 0.0,
        d: scale,
        tx: 0.0,
        ty: 0.0,
    };
    /* WHY: `matrix` は同期する CoreText 呼出しの終了まで生存し、有効な変換を表す。 */
    let path = unsafe { font.path_for_glyph(glyph_id, &matrix) }?;
    let mut state = OutlineState::new();
    /* WHY: CoreGraphics は同期的に callback を呼び、`state` と有効な element を渡す。 */
    unsafe {
        CGPath::apply(
            Some(&path),
            (&mut state as *mut OutlineState).cast(),
            Some(append_element),
        )
    };
    state.valid.then(|| state.builder.finish()).flatten()
}

fn outline_scale(font: &CTFont, units_per_em: u16) -> Option<f64> {
    /* WHY: CoreText の有効な `CTFont` から現在の point size を読むだけである。 */
    let size = unsafe { font.size() };
    let scale = f64::from(units_per_em) / size;
    (units_per_em != 0 && size.is_finite() && size > 0.0 && scale.is_finite()).then_some(scale)
}

struct OutlineState {
    builder: PathBuilder,
    valid: bool,
}

impl OutlineState {
    fn new() -> Self {
        Self {
            builder: PathBuilder::new(),
            valid: true,
        }
    }
}

unsafe extern "C-unwind" fn append_element(
    info: *mut c_void,
    element: std::ptr::NonNull<CGPathElement>,
) {
    /* WHY: `CGPath::apply` は生存中の `OutlineState` pointer と有効な path element を渡す。 */
    let state = unsafe { &mut *info.cast::<OutlineState>() };
    /* WHY: CoreGraphics は element type ごとの文書化された point count を満たす。 */
    let element = unsafe { element.as_ref() };
    state.valid &= append_path_element(&mut state.builder, element);
}

fn append_path_element(builder: &mut PathBuilder, element: &CGPathElement) -> bool {
    match element.r#type {
        CGPathElementType::MoveToPoint => point(element, 0).is_some_and(|(x, y)| {
            builder.move_to(x, y);
            true
        }),
        CGPathElementType::AddLineToPoint => point(element, 0).is_some_and(|(x, y)| {
            builder.line_to(x, y);
            true
        }),
        CGPathElementType::AddQuadCurveToPoint => append_quadratic(builder, element),
        CGPathElementType::AddCurveToPoint => append_cubic(builder, element),
        CGPathElementType::CloseSubpath => {
            builder.close();
            true
        }
        _ => false,
    }
}

fn append_quadratic(builder: &mut PathBuilder, element: &CGPathElement) -> bool {
    let (Some(control), Some(end)) = (point(element, 0), point(element, 1)) else {
        return false;
    };
    builder.quad_to(control.0, control.1, end.0, end.1);
    true
}

fn append_cubic(builder: &mut PathBuilder, element: &CGPathElement) -> bool {
    let (Some(first), Some(second), Some(end)) =
        (point(element, 0), point(element, 1), point(element, 2))
    else {
        return false;
    };
    builder.cubic_to(first.0, first.1, second.0, second.1, end.0, end.1);
    true
}

fn point(element: &CGPathElement, index: usize) -> Option<(f32, f32)> {
    /* WHY: CoreGraphics の element type が文書化済みの point count を決定する。 */
    let point: CGPoint = unsafe { *element.points.as_ptr().add(index) };
    let point = (point.x as f32, point.y as f32);
    (point.0.is_finite() && point.1.is_finite()).then_some(point)
}

#[cfg(test)]
mod tests {
    use objc2_core_foundation::CFString;
    use objc2_core_text::CTFont;

    use super::outline;

    const FONT_NAME: &str = "Helvetica";
    const GLYPH_NAME: &str = "A";
    const TEST_FONT_SIZE: f64 = 12.0;
    const FULL_UNITS_PER_EM: u16 = 1_000;
    const HALF_UNITS_PER_EM: u16 = 500;
    const MISSING_GLYPH_ID: u16 = u16::MAX;
    const INVALID_UNITS_PER_EM: u16 = 0;
    const EXPECTED_SCALE_RATIO: f32 = 2.0;
    const SCALE_TOLERANCE: f32 = 0.01;

    fn test_font() -> objc2_core_foundation::CFRetained<CTFont> {
        let name = CFString::from_str(FONT_NAME);
        /* WHY: null は CoreText が文書化する identity transform を選ぶ。 */
        unsafe { CTFont::with_name(&name, TEST_FONT_SIZE, std::ptr::null()) }
    }

    fn real_glyph(font: &CTFont) -> Result<u16, String> {
        let name = CFString::from_str(GLYPH_NAME);
        /* WHY: CoreText の glyph 名照会は有効な `CTFont` と `CFString` のみを読む。 */
        let glyph = unsafe { font.glyph_with_name(&name) };
        (glyph != 0)
            .then_some(glyph)
            .ok_or_else(|| format!("{FONT_NAME} lacks glyph {GLYPH_NAME}"))
    }

    #[test]
    fn outlines_real_glyph_in_open_type_units() -> Result<(), String> {
        let font = test_font();
        let glyph = real_glyph(&font)?;

        let path = outline(&font, glyph, FULL_UNITS_PER_EM)
            .ok_or_else(|| format!("{FONT_NAME} {GLYPH_NAME} lacks outline"))?;
        let half_path = outline(&font, glyph, HALF_UNITS_PER_EM)
            .ok_or_else(|| format!("{FONT_NAME} {GLYPH_NAME} lacks outline"))?;

        assert!(!path.is_empty());
        assert!(path.bounds().width() > 0.0);
        assert!(
            (path.bounds().width() / half_path.bounds().width() - EXPECTED_SCALE_RATIO).abs()
                < SCALE_TOLERANCE
        );
        Ok(())
    }

    #[test]
    fn rejects_missing_glyph_and_invalid_units() -> Result<(), String> {
        let font = test_font();
        let glyph = real_glyph(&font)?;

        assert!(outline(&font, MISSING_GLYPH_ID, FULL_UNITS_PER_EM).is_none());
        assert!(outline(&font, glyph, INVALID_UNITS_PER_EM).is_none());
        Ok(())
    }
}
