use super::{fallback_family, select};
use crate::markdown::svg_rasterize::text_shaping::font::{FallbackAttributes, probe_font_has_char};
use resvg::usvg::fontdb::{Stretch, Style};

#[test]
fn coretext_requires_the_exact_file_and_collection_face() -> Result<(), String> {
    let original = super::super::super::html_font_db_for_text("Helvetica, system-ui", "注");
    let face = original
        .faces()
        .find(|face| face.post_script_name == "Helvetica-Bold")
        .ok_or("Helvetica bold missing")?
        .clone();
    assert_eq!(
        fallback_family(&original, face.id, '注').as_deref(),
        Some("Hiragino Sans")
    );
    assert_rejects_altered_face(&original, face)
}

fn assert_rejects_altered_face(
    original: &resvg::usvg::fontdb::Database,
    face: resvg::usvg::fontdb::FaceInfo,
) -> Result<(), String> {
    let mut altered = original.clone();
    let mut wrong_index = face.clone();
    wrong_index.index = 0;
    let wrong_index_id = altered.push_face_info(wrong_index);
    assert_eq!(fallback_family(&altered, wrong_index_id, '注'), None);
    let mut wrong_source = face.clone();
    wrong_source.source = original
        .faces()
        .find(|f| f.post_script_name == "HiraginoSans-W5")
        .ok_or("W5 missing")?
        .source
        .clone();
    let wrong_source_id = altered.push_face_info(wrong_source);
    assert_eq!(fallback_family(&altered, wrong_source_id, '注'), None);
    let mut unknown = face;
    unknown.post_script_name = "__krr_missing_physical_postscript__".into();
    let unknown_id = altered.push_face_info(unknown);
    assert_eq!(fallback_family(&altered, unknown_id, '注'), None);
    Ok(())
}

#[test]
fn preferred_family_is_selected_from_the_current_database() -> Result<(), String> {
    let original = super::super::super::html_font_db_for_text("Helvetica, system-ui", "注");
    let base = original
        .faces()
        .find(|face| face.post_script_name == "Helvetica-Bold")
        .ok_or("Helvetica bold missing")?
        .id;
    let attributes = FallbackAttributes {
        weight: 600,
        style: Style::Normal,
        stretch: Stretch::Normal,
    };
    let chosen =
        select_from_database(&original, base, attributes).ok_or("preferred fallback missing")?;
    assert_eq!(
        original
            .face(chosen)
            .ok_or("face missing")?
            .post_script_name,
        "HiraginoSans-W5"
    );
    let mut reduced = (*original).clone();
    reduced.remove_face(chosen);
    let next = select_from_database(&reduced, base, attributes);
    assert_ne!(next, Some(chosen));
    Ok(())
}

fn select_from_database(
    database: &resvg::usvg::fontdb::Database,
    base: resvg::usvg::fontdb::ID,
    attributes: FallbackAttributes,
) -> Option<resvg::usvg::fontdb::ID> {
    select(database, base, '注', attributes, &mut |id, c| {
        probe_font_has_char(database, id, c)
    })
    .0
}
