#[cfg(target_os = "macos")]
use super::super::super::BUNDLED_SANS_SERIF_FONT;
#[cfg(target_os = "macos")]
use super::normalize_macos_hiragino_weights;

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
