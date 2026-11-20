use super::{fallback_family, select};
use crate::markdown::svg_rasterize::text_shaping::font::{FallbackAttributes, probe_font_has_char};
use objc2_core_foundation::{CFRange, CFRetained, CFString, CFURL};
use objc2_core_text::{CTFont, kCTFontURLAttribute};
use resvg::usvg::fontdb::{Source, Stretch, Style};

const BASE: &str = "Helvetica-Bold";
const GLYPH: char = '注';

#[test]
fn coretext_requires_the_exact_file_and_collection_face() -> Result<(), String> {
    let original = super::super::super::html_font_db_for_text("Helvetica, system-ui", "注");
    let face = original
        .faces()
        .find(|face| face.post_script_name == BASE)
        .ok_or("Helvetica bold missing")?
        .clone();
    let reference = reference_fallback();
    let expected = unsafe { reference.family_name() }.to_string();
    assert_eq!(fallback_family(&original, face.id, GLYPH), Some(expected));
    assert_rejects_altered_face(&original, face, font_path(&reference)?)
}

fn assert_rejects_altered_face(
    original: &resvg::usvg::fontdb::Database,
    face: resvg::usvg::fontdb::FaceInfo,
    wrong_path: std::path::PathBuf,
) -> Result<(), String> {
    let mut altered = original.clone();
    let mut wrong_index = face.clone();
    wrong_index.index ^= 1;
    let wrong_index_id = altered.push_face_info(wrong_index);
    assert_eq!(fallback_family(&altered, wrong_index_id, GLYPH), None);
    let mut wrong_source = face.clone();
    wrong_source.source = Source::File(wrong_path);
    let wrong_source_id = altered.push_face_info(wrong_source);
    assert_eq!(fallback_family(&altered, wrong_source_id, GLYPH), None);
    for name in ["", "__krr_missing_physical_postscript__"] {
        let mut unknown = face.clone();
        unknown.post_script_name = name.into();
        let unknown_id = altered.push_face_info(unknown);
        assert_eq!(fallback_family(&altered, unknown_id, GLYPH), None);
    }
    Ok(())
}

#[test]
fn preferred_family_is_selected_from_the_current_database() -> Result<(), String> {
    let original = super::super::super::html_font_db_for_text("Helvetica, system-ui", "注");
    let mut database = original.as_ref().clone();
    let base = original
        .faces()
        .find(|face| face.post_script_name == BASE)
        .ok_or("Helvetica bold missing")?
        .id;
    let family = register_reference_family(&mut database)?;
    let attributes = FallbackAttributes {
        weight: 600,
        style: Style::Normal,
        stretch: Stretch::Normal,
    };
    let chosen =
        select_from_database(&database, base, attributes).ok_or("preferred fallback missing")?;
    assert!(
        database
            .face(chosen)
            .ok_or("face missing")?
            .families
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(&family))
    );
    database.remove_face(chosen);
    let next = select_from_database(&database, base, attributes);
    assert_ne!(next, Some(chosen));
    Ok(())
}

fn register_reference_family(
    database: &mut resvg::usvg::fontdb::Database,
) -> Result<String, String> {
    let reference = reference_fallback();
    database
        .load_font_file(font_path(&reference)?)
        .map_err(|error| error.to_string())?;
    Ok(unsafe { reference.family_name() }.to_string())
}

fn select_from_database(
    database: &resvg::usvg::fontdb::Database,
    base: resvg::usvg::fontdb::ID,
    attributes: FallbackAttributes,
) -> Option<resvg::usvg::fontdb::ID> {
    select(database, base, GLYPH, attributes, &mut |id, c| {
        probe_font_has_char(database, id, c)
    })
    .0
}

fn reference_fallback() -> CFRetained<CTFont> {
    let name = CFString::from_str(BASE);
    let base = unsafe { CTFont::with_name(&name, super::FALLBACK_FONT_SIZE, std::ptr::null()) };
    assert_eq!(unsafe { base.post_script_name() }.to_string(), BASE);
    let glyph = CFString::from_str(&GLYPH.to_string());
    let range = CFRange {
        location: 0,
        length: glyph.length(),
    };
    unsafe { base.for_string(&glyph, range) }
}

fn font_path(font: &CTFont) -> Result<std::path::PathBuf, String> {
    let descriptor = unsafe { font.font_descriptor() };
    let value = unsafe { descriptor.attribute(kCTFontURLAttribute) };
    value
        .and_then(|value| value.downcast::<CFURL>().ok()?.to_file_path())
        .ok_or_else(|| "system fallback file missing".into())
}
