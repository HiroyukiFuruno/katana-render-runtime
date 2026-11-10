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

#[test]
fn missing_alias_glyph_requires_non_interface_fallback() -> Result<(), String> {
    let (database, base, alias) = test_support::shared_file_alias_fixture()?;
    let face = database
        .face(base)
        .ok_or("system-ui primary face missing")?;
    assert!(super::super::super::font::is_system_ui_primary(face));
    assert_eq!(
        super::super::super::font::bold_alias_id(&database),
        Some(alias)
    );
    let character = alias_character(&database, alias, false)?;
    let attributes = super::fallback_attributes(super::INTERFACE_BOLD_WEIGHT, false);
    let mut probe = |id, glyph| super::probe_font_has_char(&database, id, glyph);
    assert_eq!(
        super::requested_preflight(&database, base, character, attributes, &mut probe),
        Ok(true)
    );
    assert_real_non_interface_fallback(&database, base, character);
    Ok(())
}

#[test]
fn unreadable_alias_disables_requested_fallback_and_caching() -> Result<(), String> {
    use resvg::usvg::fontdb::Source;

    let (mut database, base, alias) = test_support::shared_file_alias_fixture()?;
    let mut face = database
        .face(alias)
        .ok_or("interface alias missing")?
        .clone();
    let Source::SharedFile(path, _) = &face.source else {
        return Err("interface fixture is not shared-file backed".into());
    };
    face.source = Source::File(path.join("unavailable-font"));
    database.remove_face(alias);
    let alias = database.push_face_info(face);
    assert_eq!(
        super::super::super::font::bold_alias_id(&database),
        Some(alias)
    );
    assert_eq!(super::probe_font_has_char(&database, alias, 'A'), None);
    let attributes = super::fallback_attributes(super::INTERFACE_BOLD_WEIGHT, false);
    let mut probe = |id, glyph| super::probe_font_has_char(&database, id, glyph);
    assert_eq!(
        super::requested_preflight(&database, base, 'A', attributes, &mut probe),
        Err((None, false))
    );
    Ok(())
}

fn assert_real_non_interface_fallback(
    database: &resvg::usvg::fontdb::Database,
    base: resvg::usvg::fontdb::ID,
    character: char,
) {
    let (selected, _) = super::matching_fallback_with_probe(
        database,
        base,
        character,
        super::INTERFACE_BOLD_WEIGHT,
        false,
        |id, glyph| super::probe_font_has_char(database, id, glyph),
    );
    if let Some(id) = selected {
        assert!(
            !database
                .face(id)
                .is_some_and(super::super::super::font::is_interface_face)
        );
        assert_eq!(
            super::probe_font_has_char(database, id, character),
            Some(true)
        );
    }
}

#[test]
fn interface_fallback_is_excluded_only_when_required() -> Result<(), String> {
    let (mut database, base, alias) = test_support::shared_file_alias_fixture()?;
    assert!(
        database
            .face(alias)
            .is_some_and(super::super::super::font::is_interface_face)
    );
    let character = alias_character(&database, alias, true)?;
    retain_primary_and_alias(&mut database, base, alias);
    let attributes = super::fallback_attributes(super::INTERFACE_BOLD_WEIGHT, false);
    let selected = |required| {
        super::matching_fallback_regular(
            &database,
            base,
            character,
            attributes,
            required,
            |id, glyph| super::probe_font_has_char(&database, id, glyph),
        )
    };
    assert_eq!(selected(false), (Some(alias), true));
    assert_eq!(selected(true), (None, true));
    Ok(())
}

#[test]
fn duplicate_native_bold_alias_disables_ambiguous_fallback_and_cache() -> Result<(), String> {
    let (mut database, base, alias) = test_support::shared_file_alias_fixture()?;
    retain_primary_and_alias(&mut database, base, alias);
    let character = alias_character(&database, alias, true)?;
    let select = |database: &resvg::usvg::fontdb::Database| {
        super::matching_fallback_with_probe(
            database,
            base,
            character,
            super::INTERFACE_BOLD_WEIGHT,
            false,
            |id, glyph| super::probe_font_has_char(database, id, glyph),
        )
    };
    assert_eq!(select(&database), (Some(alias), true));
    let duplicate = database
        .face(alias)
        .ok_or("interface alias missing")?
        .clone();
    assert_ne!(database.push_face_info(duplicate), alias);
    assert!(super::super::super::font::has_native_interface_candidate(
        &database
    ));
    assert_eq!(select(&database), (None, false));
    Ok(())
}

fn retain_primary_and_alias(
    database: &mut resvg::usvg::fontdb::Database,
    base: resvg::usvg::fontdb::ID,
    alias: resvg::usvg::fontdb::ID,
) {
    let others: Vec<_> = database
        .faces()
        .map(|face| face.id)
        .filter(|id| *id != base && *id != alias)
        .collect();
    for id in others {
        database.remove_face(id);
    }
}

fn alias_character(
    database: &resvg::usvg::fontdb::Database,
    alias: resvg::usvg::fontdb::ID,
    supported: bool,
) -> Result<char, String> {
    ['A', 'B', '日', '本', '\u{10ffff}', '\u{10fffe}']
        .into_iter()
        .find(|character| {
            super::probe_font_has_char(database, alias, *character) == Some(supported)
        })
        .ok_or_else(|| format!("real interface face has no character with support={supported}"))
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
