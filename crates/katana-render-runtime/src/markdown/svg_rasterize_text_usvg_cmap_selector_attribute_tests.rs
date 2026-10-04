use super::super::tests::FONT_BYTES;
use super::same_fallback_attributes;
use resvg::usvg::fontdb::{Database, FaceInfo, Stretch, Style, Weight};

fn bundled_face() -> Result<FaceInfo, String> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let mut face = database
        .faces()
        .next()
        .ok_or_else(|| "bundled font should contain a face".to_string())?
        .clone();
    face.style = Style::Normal;
    face.weight = Weight::NORMAL;
    face.stretch = Stretch::Normal;
    Ok(face)
}

fn face_with_attributes(
    base: &FaceInfo,
    style: Style,
    weight: Weight,
    stretch: Stretch,
) -> FaceInfo {
    let mut face = base.clone();
    face.style = style;
    face.weight = weight;
    face.stretch = stretch;
    face
}

#[test]
fn fallback_attribute_comparison_preserves_usvg_conjunction_semantics() -> Result<(), String> {
    let base = bundled_face()?;

    let same = face_with_attributes(&base, Style::Normal, Weight::NORMAL, Stretch::Normal);
    assert!(same_fallback_attributes(&base, &same));

    let style_only = face_with_attributes(&base, Style::Italic, Weight::NORMAL, Stretch::Normal);
    assert!(same_fallback_attributes(&base, &style_only));

    let style_and_weight =
        face_with_attributes(&base, Style::Italic, Weight::BOLD, Stretch::Normal);
    assert!(same_fallback_attributes(&base, &style_and_weight));

    let all_differ = face_with_attributes(&base, Style::Italic, Weight::BOLD, Stretch::Condensed);
    assert!(!same_fallback_attributes(&base, &all_differ));
    Ok(())
}
