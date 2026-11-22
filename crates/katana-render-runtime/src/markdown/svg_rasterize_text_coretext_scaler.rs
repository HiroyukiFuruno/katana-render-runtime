use objc2_core_foundation::{CFDictionary, CFNumber, CFRetained, CFString};
use objc2_core_text::{CTFont, CTFontDescriptor, kCTFontOpticalSizeAttribute};
use resvg::usvg::fontdb::{FaceInfo, Source};

use super::font_identity;

const ZERO_UNSCALED_TRACKING: i32 = 0;

pub(super) fn matches_copied_face(
    font: &CTFont,
    face: &FaceInfo,
    bytes: &[u8],
    index: u32,
) -> bool {
    let path = match &face.source {
        Source::File(path) | Source::SharedFile(path, _) => path,
        Source::Binary(_) => return false,
    };
    let Some(actual) = font_identity::actual_font_metadata(font) else {
        return false;
    };
    face.index == index
        && actual.canonical_path == *path
        && actual.post_script_name == face.post_script_name
        && font_identity::face_index(bytes, &actual.post_script_name, font) == Some(index)
}

pub(super) fn scaler_font(font: &CTFont) -> Option<CFRetained<CTFont>> {
    let font_size = font_size(font)?;
    let optical_size = configured_optical_size(font).unwrap_or(font_size);
    let optical_size_value = CFNumber::new_f64(optical_size);
    let tracking_value = CFNumber::new_i32(ZERO_UNSCALED_TRACKING);
    /* SAFETY: CoreTextの公開属性定数はプロセスの存続中有効。 */
    let optical_attribute = unsafe { kCTFontOpticalSizeAttribute };
    /* WHY: SkiaのmacOS描画と同じく、字形位置へCoreTextの自動trackingを重ねない。 */
    let tracking_attribute = CFString::from_str("NSCTFontUnscaledTrackingAttribute");
    let attributes = CFDictionary::from_slices(
        &[optical_attribute, &tracking_attribute],
        &[&*optical_size_value, &*tracking_value],
    );
    /* SAFETY: 属性辞書はCFString/CFNumberだけで構成し、CoreTextが要求する型と一致する。 */
    let descriptor = unsafe { CTFontDescriptor::with_attributes((*attributes).as_ref()) };
    /* SAFETY: null matrixは元の変換を維持する公開CoreText契約である。 */
    Some(unsafe { font.copy_with_attributes(font_size, std::ptr::null(), Some(&descriptor)) })
}

fn configured_optical_size(font: &CTFont) -> Option<f64> {
    /* SAFETY: CoreTextが返す所有済み属性は、有効期間中にCFNumberへ型検査する。 */
    unsafe { font.attribute(kCTFontOpticalSizeAttribute) }
        .and_then(|value| value.downcast::<CFNumber>().ok())
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn font_size(font: &CTFont) -> Option<f64> {
    /* SAFETY: CTFontは呼出元が有効な参照として渡し、sizeの取得は状態を変更しない。 */
    let size = unsafe { font.size() };
    (size.is_finite() && size > 0.0).then_some(size)
}

#[cfg(test)]
mod tests {
    use super::scaler_font;
    use objc2_core_foundation::CFString;
    use objc2_core_text::{CTFont, CTFontUIFontType};

    const TEST_POINT_SIZE: f64 = 16.0;

    #[test]
    fn scaler_copy_preserves_a_positive_font_size() -> Result<(), String> {
        let language = CFString::from_str("zh-Hans");
        /* SAFETY: 固定の有効なUI種別・正のpoint size・所有中のCFStringだけをCoreTextへ渡す。 */
        let font = unsafe {
            CTFont::new_ui_font_for_language(
                CTFontUIFontType::System,
                TEST_POINT_SIZE,
                Some(&language),
            )
        }
        .ok_or("CoreText system UI font unavailable")?;
        let scaler = scaler_font(&font).ok_or("CoreText scaler copy unavailable")?;
        /* SAFETY: scalerは直前のCoreText copy APIが返した有効なCTFontである。 */
        let copied_size = unsafe { scaler.size() };
        if !copied_size.is_finite() || copied_size <= 0.0 {
            return Err("CoreText scaler copy returned an invalid size".to_string());
        }
        if (copied_size - TEST_POINT_SIZE).abs() > f64::EPSILON {
            return Err("CoreText scaler copy changed the requested size".to_string());
        }
        Ok(())
    }
}
