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
    use super::{CTFont, matches_copied_face, scaler_font};
    use core_foundation::base::TCFType;
    use core_graphics::{data_provider::CGDataProvider, font::CGFont};
    use core_text::font::new_from_CGFont;
    use objc2_core_foundation::CFString;
    use objc2_core_text::CTFontUIFontType;
    use resvg::usvg::fontdb::{Database, FaceInfo, Source};
    use std::{path::Path, sync::Arc};

    const HELVETICA_FAMILY: &str = "Helvetica";
    const NOTO_FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
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
        assert!(
            copied_size.is_finite() && copied_size > 0.0,
            "CoreText scaler copy returned an invalid size"
        );
        assert!(
            (copied_size - TEST_POINT_SIZE).abs() <= f64::EPSILON,
            "CoreText scaler copy changed the requested size"
        );
        Ok(())
    }

    #[test]
    fn scaler_copy_binds_the_physical_face_and_rejects_unbound_variants() -> Result<(), String> {
        let original = helvetica_font();
        let actual = super::super::font_identity::resolve_actual_face(&original)
            .ok_or("CoreText font has no physical face")?;
        let scaler = scaler_font(&original).ok_or("CoreText scaler copy unavailable")?;
        let mut database = Database::new();
        database.load_font_source(Source::File(actual.canonical_path.clone()));
        let id = database
            .faces()
            .find(|face| {
                face.index == actual.face_index && face.post_script_name == actual.post_script_name
            })
            .map(|face| face.id)
            .ok_or("CoreText physical face missing from its file")?;
        let face = database.face(id).ok_or("physical face unavailable")?;
        database
            .with_face_data(id, |bytes, index| {
                assert!(matches_copied_face(&scaler, face, bytes, index));
                assert_unbound_variants_rejected(
                    &scaler,
                    face,
                    bytes,
                    index,
                    &actual.canonical_path,
                )
            })
            .ok_or("physical face bytes unavailable")?
    }

    #[test]
    fn scaler_copy_rejects_a_memory_backed_coretext_font() -> Result<(), String> {
        let provider = CGDataProvider::from_buffer(Arc::new(NOTO_FONT.to_vec()));
        let graphics_font =
            CGFont::from_data_provider(provider).map_err(|_| "CGFont creation failed")?;
        let memory_font = new_from_CGFont(&graphics_font, TEST_POINT_SIZE);
        /* WHY: CGFont所有者が生存する間だけ、同じCTFontをobjc2側で借用する。 */
        let font = unsafe { &*memory_font.as_concrete_TypeRef().cast::<CTFont>() };
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf");
        let mut database = Database::new();
        database.load_font_source(Source::File(path));
        let id = database
            .faces()
            .next()
            .ok_or("bundled font face missing")?
            .id;
        let face = database.face(id).ok_or("bundled font face unavailable")?;
        database
            .with_face_data(id, |bytes, index| {
                assert!(!matches_copied_face(font, face, bytes, index));
            })
            .ok_or("bundled font bytes unavailable")?;
        Ok(())
    }

    fn assert_unbound_variants_rejected(
        scaler: &CTFont,
        face: &FaceInfo,
        bytes: &[u8],
        index: u32,
        path: &Path,
    ) -> Result<(), String> {
        let mut renamed = face.clone();
        renamed.post_script_name = "invalid-Helvetica-name".to_string();
        let mut moved = face.clone();
        moved.source = Source::File(path.with_extension("mismatch"));
        let mut binary = face.clone();
        binary.source = Source::Binary(Arc::new(bytes.to_vec()));
        let other_index = index.checked_add(1).ok_or("physical face index overflow")?;
        for (case, candidate, candidate_bytes, candidate_index) in [
            ("name", &renamed, bytes, index),
            ("path", &moved, bytes, index),
            ("index", face, bytes, other_index),
            ("bytes", face, &[][..], index),
            ("binary source", &binary, bytes, index),
        ] {
            assert!(
                !matches_copied_face(scaler, candidate, candidate_bytes, candidate_index),
                "CoreText scaler copy accepted a mismatch in {case}"
            );
        }
        Ok(())
    }

    fn helvetica_font() -> objc2_core_foundation::CFRetained<CTFont> {
        let name = CFString::from_str(HELVETICA_FAMILY);
        /* SAFETY: CoreTextへ固定の既知family名・正のpoint size・null transformだけを渡す。 */
        unsafe { CTFont::with_name(&name, TEST_POINT_SIZE, std::ptr::null()) }
    }
}
