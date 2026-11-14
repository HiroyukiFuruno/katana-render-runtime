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
fn cjk_fallback_registers_w4_in_eager_and_known_databases() -> Result<(), String> {
    use crate::markdown::svg_rasterize::font::HtmlFontRequest;

    let request = HtmlFontRequest::from_text("Helvetica", "注文サービス");
    let eager = super::build_html_font_db(&request);
    let (known, proof) = super::build_known_html_font_db(&request);
    assert!(proof.current(&known));
    for database in [&eager, &known] {
        let face = database
            .faces()
            .find(|face| face.post_script_name == "HiraginoSans-W4")
            .ok_or("CJK fallback W4 missing")?;
        assert_eq!(face.weight.0, 500);
    }
    let latin = super::build_html_font_db(&HtmlFontRequest::from_text("Helvetica", "Order"));
    assert!(
        !latin
            .faces()
            .any(|face| face.post_script_name == "HiraginoSans-W4")
    );
    Ok(())
}

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
fn coretext_interface_identity_propagates_no_font_for_the_no_font_selector() {
    use super::interface_bold::coretext_interface_identity;
    use objc2_core_text::CTFontUIFontType;

    assert!(coretext_interface_identity(CTFontUIFontType::None).is_none());
    assert!(coretext_interface_identity(CTFontUIFontType::EmphasizedSystem).is_some());
}

#[cfg(target_os = "macos")]
#[test]
fn coretext_regular_interface_identity_cannot_register_a_bold_alias() -> Result<(), String> {
    use super::interface_bold::{coretext_face_identity, register_coretext_identity};

    let mut database = resvg::usvg::fontdb::Database::new();
    super::load_font_paths(&mut database, super::SYSTEM_CJK_FONT_PATHS);
    let (regular, bold) = japanese_ui_font_pair()?;
    let regular = coretext_face_identity(&regular).ok_or("CoreText regular identity missing")?;
    let bold = coretext_face_identity(&bold).ok_or("CoreText bold identity missing")?;
    let face_count = database.len();
    assert!(register_coretext_identity(&mut database, regular).is_none());
    assert_eq!(database.len(), face_count);
    let alias = register_coretext_identity(&mut database, bold).ok_or("CoreText bold rejected")?;
    assert_eq!(bold_alias_id(&database), Some(alias));
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn coretext_interface_identity_rejects_a_font_without_a_file_url() -> Result<(), String> {
    use core_foundation::base::TCFType;
    use core_graphics::{data_provider::CGDataProvider, font::CGFont};
    use objc2_core_text::CTFont;
    use std::sync::Arc;

    let data = CGDataProvider::from_buffer(Arc::new(BUNDLED_SANS_SERIF_FONT.to_vec()));
    let graphics = CGFont::from_data_provider(data).map_err(|_| "CGFont creation failed")?;
    let font = core_text::font::new_from_CGFont(&graphics, 12.0);
    /* WHY: CTFontの所有者が生存する間だけ、同じCoreTextオブジェクトを借用する。 */
    let font = unsafe { &*font.as_concrete_TypeRef().cast::<CTFont>() };
    let descriptor = unsafe { font.font_descriptor() };
    assert!(unsafe { descriptor.attribute(objc2_core_text::kCTFontNameAttribute) }.is_some());
    assert!(unsafe { descriptor.attribute(objc2_core_text::kCTFontURLAttribute) }.is_none());
    assert!(super::interface_bold::coretext_face_identity(font).is_none());
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
fn interface_bold_rejects_an_in_memory_copy_claiming_the_native_alias() -> Result<(), String> {
    use resvg::usvg::fontdb::{Database, Language, Source};
    use std::sync::Arc;

    let mut database = Database::new();
    super::load_font_paths(&mut database, super::SYSTEM_CJK_FONT_PATHS);
    let mut face = database
        .faces()
        .find(|face| face.post_script_name == ".HiraKakuInterface-W6")
        .ok_or("native interface bold face missing")?
        .clone();
    let native_id = face.id;
    let bytes = database
        .with_face_data(native_id, |data, _| data.to_vec())
        .ok_or("native interface bold font data unavailable")?;
    face.source = Source::Binary(Arc::new(bytes));
    face.families.push((
        INTERNAL_INTERFACE_BOLD_FAMILY.into(),
        Language::English_UnitedStates,
    ));
    let copy_id = database.push_face_info(face);

    assert!(bold_alias_id(&database).is_none());
    assert!(register_interface_bold(&mut database).is_none());
    assert!(database.face(native_id).is_some());
    assert!(database.face(copy_id).is_some());
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn interface_bold_registration_rejects_database_without_native_face() {
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_data(super::super::super::BUNDLED_SANS_SERIF_FONT.to_vec());
    assert!(register_interface_bold(&mut database).is_none());
}
