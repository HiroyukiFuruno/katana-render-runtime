use super::*;

fn assert_probe_reuse_matches_file_durability(
    database: &Arc<Database>,
    face_id: ID,
    font_path: &Path,
) -> Result<(), String> {
    let durable = super::super::super::super::font::file_stamp_durable_reusable(font_path);
    let (first_probe, subsequent_probe) = with_validated_scope(|| {
        (
            super::super::has_char(database, face_id, 'A'),
            super::super::has_char(database, face_id, 'A'),
        )
    });
    if durable {
        assert_eq!(first_probe, Ok(true));
        assert_eq!(subsequent_probe, Ok(true));
    } else {
        assert_eq!(first_probe, Err(()));
        assert_eq!(subsequent_probe, Err(()));
        let generation =
            super::super::super::super::font::font_source_generation(database, face_id);
        assert!(
            super::super::super::storage::lookup(database, face_id, 0, &generation,)
                .map_err(|_| "cmap cache lock poisoned")?
                .is_none()
        );
    }
    Ok(())
}

#[test]
fn selector_falls_back_to_stock_when_cached_probe_is_unavailable() -> Result<(), String> {
    const CASE: &str = "markdown::svg_rasterize::text_shaping::usvg_cmap::probe::tests::selector_tests::selector_falls_back_to_stock_when_cached_probe_is_unavailable";
    if !super::super::super::storage::tests::global_child::run_parent_or_allow_child(CASE)? {
        return Ok(());
    }
    let (_lock, _reset) = super::cache_test_guard()?;
    let first_font = super::TempFont::write(super::FONT_BYTES)?;
    let second_font = super::TempFont::write(super::FONT_BYTES)?;
    let (database, base_face, fallback_face) =
        super::database_with_two_file_fonts(&first_font, &second_font)?;
    assert_eq!(
        super::super::has_char(&database, fallback_face, 'A'),
        Err(())
    );
    assert_probe_reuse_matches_file_durability(&database, fallback_face, &second_font.0)?;

    let excluded = [base_face];
    let stock = resvg::usvg::FontResolver::default_fallback_selector();
    let cached = super::super::super::selector::html_selector();
    let (stock_result, cached_result) = super::with_validated_scope(|| {
        (
            stock('A', &excluded, &mut Arc::clone(&database)),
            cached('A', &excluded, &mut Arc::clone(&database)),
        )
    });
    assert_eq!(stock_result, Some(fallback_face));
    assert_eq!(cached_result, stock_result);
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn html_unicode_fallback_selects_hiragino_w3_for_circled_digit() -> Result<(), String> {
    const CASE: &str = "markdown::svg_rasterize::text_shaping::usvg_cmap::probe::tests::selector_tests::html_unicode_fallback_selects_hiragino_w3_for_circled_digit";
    if !super::super::super::storage::tests::global_child::run_parent_or_allow_child(CASE)? {
        return Ok(());
    }
    html_unicode_fallback_selects_hiragino_w3_for_circled_digit_body()
}

#[cfg(target_os = "macos")]
#[test]
fn html_resolved_family_query_matches_chrome_normal_and_bold_weights() -> Result<(), String> {
    let database = crate::markdown::svg_rasterize::text_shaping::html_font_db_for_text(
        "Hiragino Sans",
        "日本",
    );
    for (weight, expected_post_script_name) in [(400, "HiraginoSans-W3"), (700, "HiraginoSans-W6")]
    {
        let selected = database
            .query(&resvg::usvg::fontdb::Query {
                families: &[resvg::usvg::fontdb::Family::Name("Hiragino Sans")],
                weight: resvg::usvg::fontdb::Weight(weight),
                stretch: resvg::usvg::fontdb::Stretch::Normal,
                style: resvg::usvg::fontdb::Style::Normal,
            })
            .ok_or_else(|| format!("missing Hiragino face for weight {weight}"))?;
        let face = database.face(selected).ok_or("queried face missing")?;
        assert_eq!(face.post_script_name, expected_post_script_name);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn html_unicode_fallback_selects_hiragino_w3_for_circled_digit_body() -> Result<(), String> {
    let (_lock, _reset) = super::cache_test_guard()?;
    let database =
        crate::markdown::svg_rasterize::text_shaping::html_font_db_for_text("KrrNoto", "①");
    let base_face = database
        .faces()
        .find(|face| {
            face.families
                .iter()
                .any(|(family, _)| family == "Noto Sans")
        })
        .ok_or("bundled Noto Sans face missing")?;
    let selector = super::super::super::selector::html_selector();
    let selected =
        super::with_validated_scope(|| selector('①', &[base_face.id], &mut Arc::clone(&database)))
            .ok_or("circled digit fallback face missing")?;
    let face = database
        .face(selected)
        .ok_or("selected fallback face missing")?;
    let resvg::usvg::fontdb::Source::File(path) = &face.source else {
        return Err("fallback face is not file-backed".into());
    };
    if !path.ends_with("ヒラギノ角ゴシック W3.ttc") {
        return Err(format!("unexpected fallback face: {}", path.display()));
    }
    Ok(())
}
