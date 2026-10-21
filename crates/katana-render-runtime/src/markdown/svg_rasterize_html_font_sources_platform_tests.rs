#[cfg(target_os = "macos")]
use super::super::super::BUNDLED_SANS_SERIF_FONT;
#[cfg(target_os = "macos")]
use super::interface_bold::INTERNAL_INTERFACE_BOLD_FAMILY;
#[cfg(target_os = "macos")]
use super::normalize_macos_hiragino_weights;
#[cfg(target_os = "macos")]
use super::{bold_alias_id, register_interface_bold};
#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_font_interface_bold_test_support.rs"]
mod interface_bold_support;
#[cfg(target_os = "macos")]
use interface_bold_support::{assert_coretext_interface_face, japanese_ui_font_pair};

#[cfg(target_os = "macos")]
#[test]
fn embedded_hiragino_named_fonts_keep_their_declared_weight() -> Result<(), String> {
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_data(BUNDLED_SANS_SERIF_FONT.to_vec());
    let mut face = database
        .faces()
        .next()
        .ok_or("embedded face missing")?
        .clone();
    let old_id = face.id;
    face.post_script_name = "HiraginoSans-W3".into();
    face.families = vec![(
        "Hiragino Sans".into(),
        resvg::usvg::fontdb::Language::English_UnitedStates,
    )];
    face.weight = resvg::usvg::fontdb::Weight(250);
    database.remove_face(old_id);
    let id = database.push_face_info(face);
    normalize_macos_hiragino_weights(&mut database);
    assert_eq!(
        database.face(id).ok_or("custom face must remain")?.weight.0,
        250
    );
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn coretext_interface_family_selects_w4_regular_and_w6_bold() -> Result<(), String> {
    let (regular, bold) = japanese_ui_font_pair()?;
    assert_coretext_interface_face(&regular, ".HiraKakuInterface-W4", "W4.ttc", false)?;
    assert_coretext_interface_face(&bold, ".HiraKakuInterface-W6", "W6.ttc", true)?;
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn known_system_ui_database_binds_bold_alias_to_coretext_physical_face() -> Result<(), String> {
    use crate::markdown::svg_rasterize::font::HtmlFontRequest;

    let request = HtmlFontRequest::from_text("system-ui", "日本");
    let (mut database, proof) = super::build_known_html_font_db(&request);
    let id = bold_alias_id(&database).ok_or("CoreText interface bold alias missing")?;
    let face = database
        .face(id)
        .ok_or("CoreText interface bold face missing")?;
    assert_eq!(face.post_script_name, ".HiraKakuInterface-W6");
    assert_eq!(face.weight.0, 600);
    assert!(
        face.families
            .iter()
            .any(|(name, _)| name == INTERNAL_INTERFACE_BOLD_FAMILY)
    );
    assert!(proof.current(&database));
    assert_eq!(register_interface_bold(&mut database), Some(id));
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn known_interface_alias_survives_proof_validated_shared_file_mapping() -> Result<(), String> {
    use crate::markdown::svg_rasterize::font::HtmlFontRequest;
    use std::sync::Arc;

    let request = HtmlFontRequest::from_text("system-ui", "日本");
    let (database, proof) = super::build_known_html_font_db(&request);
    let original = Arc::new(database);
    let original_id = bold_alias_id(&original).ok_or("interface alias missing")?;
    let (shared, shared_proof) = proof.transient(&original).ok_or("shared proof failed")?;
    let shared_id = bold_alias_id(&shared).ok_or("shared interface alias missing")?;
    let original_face = original
        .face(original_id)
        .ok_or("original interface alias missing")?;
    let face = shared
        .face(shared_id)
        .ok_or("remapped shared interface alias missing")?;
    assert_shared_interface_alias_mapping(original_face, face)?;
    assert!(super::has_native_interface_candidate(&shared));
    assert!(shared_proof.current(&shared));
    Ok(())
}

#[cfg(target_os = "macos")]
fn assert_shared_interface_alias_mapping(
    original_face: &resvg::usvg::fontdb::FaceInfo,
    face: &resvg::usvg::fontdb::FaceInfo,
) -> Result<(), String> {
    let (
        resvg::usvg::fontdb::Source::File(original_path),
        resvg::usvg::fontdb::Source::SharedFile(shared_path, _),
    ) = (&original_face.source, &face.source)
    else {
        return Err("interface alias source mapping did not become SharedFile".into());
    };
    assert_eq!(shared_path, original_path);
    assert_eq!(face.post_script_name, original_face.post_script_name);
    assert_eq!(face.index, original_face.index);
    assert_eq!(face.weight, original_face.weight);
    assert_eq!(face.style, original_face.style);
    assert_eq!(face.families, original_face.families);
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn interface_bold_registration_rejects_ambiguous_physical_identity() -> Result<(), String> {
    let mut database = resvg::usvg::fontdb::Database::new();
    super::load_font_paths(&mut database, super::SYSTEM_CJK_FONT_PATHS);
    let face = database
        .faces()
        .find(|face| face.post_script_name == ".HiraKakuInterface-W6")
        .ok_or("native interface bold face missing")?
        .clone();
    database.push_face_info(face);
    assert!(register_interface_bold(&mut database).is_none());
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn interface_bold_registration_rejects_database_without_native_face() {
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_data(super::super::super::BUNDLED_SANS_SERIF_FONT.to_vec());
    assert!(register_interface_bold(&mut database).is_none());
}
