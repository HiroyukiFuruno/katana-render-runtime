use super::matching_font_face;
use crate::markdown::svg_rasterize::font::html_font_db_for_text;

#[path = "svg_rasterize_text_font_interface_bold_test_support.rs"]
mod test_support;

#[test]
fn reserved_internal_interface_family_cannot_be_selected_by_author_name() {
    let database = html_font_db_for_text("system-ui", "日本");
    assert!(
        matching_font_face(
            &database,
            "__krr_internal_system_ui_interface_bold_700",
            700,
            false
        )
        .is_none()
    );
}

#[test]
fn interface_family_bold_uses_the_coretext_alias_id() -> Result<(), String> {
    let database = html_font_db_for_text("system-ui", "日本");
    let id = matching_font_face(&database, ".Hiragino Kaku Gothic Interface", 700, false)
        .ok_or("CoreText interface alias was not selected")?;
    let face = database.face(id).ok_or("selected interface face missing")?;
    assert_eq!(face.post_script_name, ".HiraKakuInterface-W6");
    Ok(())
}

#[test]
fn explicit_native_interface_uses_coretext_bold_without_system_ui_primary() -> Result<(), String> {
    let database = html_font_db_for_text(".Hiragino Kaku Gothic Interface", "日本");
    let alias = super::super::super::font::bold_alias_id(&database)
        .ok_or("CoreText interface bold alias was not registered")?;
    assert_eq!(
        database
            .face(alias)
            .ok_or("CoreText interface alias face is missing")?
            .post_script_name,
        ".HiraKakuInterface-W6"
    );
    assert_eq!(
        matching_font_face(&database, ".Hiragino Kaku Gothic Interface", 700, false),
        Some(alias)
    );
    Ok(())
}

#[test]
fn system_ui_bold_fallback_keeps_registered_alias_after_shared_file_conversion()
-> Result<(), String> {
    let (database, base, alias) = test_support::shared_file_alias_fixture()?;
    let (selected, cacheable) =
        super::matching_fallback_with_probe(&database, base, '日', 700, false, |id, character| {
            Some(super::font_has_char(&database, id, character))
        });
    assert_selected_alias(&database, base, alias, selected);
    assert!(cacheable);
    Ok(())
}

fn assert_selected_alias(
    database: &resvg::usvg::fontdb::Database,
    base: resvg::usvg::fontdb::ID,
    alias: resvg::usvg::fontdb::ID,
    selected: Option<resvg::usvg::fontdb::ID>,
) {
    assert_eq!(
        selected,
        Some(alias),
        "base={}; expected_alias={}; selected={}",
        face_diagnostic(database.face(base)),
        face_diagnostic(database.face(alias)),
        face_diagnostic(selected.and_then(|id| database.face(id)))
    );
}

fn face_diagnostic(face: Option<&resvg::usvg::fontdb::FaceInfo>) -> String {
    let Some(face) = face else {
        return "missing".into();
    };
    let path = match &face.source {
        resvg::usvg::fontdb::Source::File(path)
        | resvg::usvg::fontdb::Source::SharedFile(path, _) => path.display().to_string(),
        _ => "other-source".into(),
    };
    format!(
        "PS={} families={:?} index={} weight={} path={}",
        face.post_script_name, face.families, face.index, face.weight.0, path
    )
}
