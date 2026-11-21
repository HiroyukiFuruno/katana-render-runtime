use super::{HtmlTextMeasurementToken, MAX_FAMILY_BYTES};

#[cfg(unix)]
#[test]
fn token_key_is_cloneable_and_usable_inside_scope() {
    super::super::with_render_scoped_stamp_batch(|| {
        let token = HtmlTextMeasurementToken::for_request("A", "Noto Sans");
        assert!(token.is_some());
        if let Some(token) = token {
            assert_eq!(token.key(), token.clone().key());
            assert!(token.is_usable());
        }
    });
}

#[cfg(windows)]
#[test]
fn windows_file_font_generation_disables_token_reuse() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf");
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_source(resvg::usvg::fontdb::Source::File(path.clone()));
    let database = std::sync::Arc::new(database);
    super::super::with_render_scoped_stamp_batch(|| {
        assert!(!super::super::font::file_stamp_durable_reusable(&path));
        assert!(super::render_generations(&database).is_none());
        let key = super::DatabaseKey::eager(super::HtmlFontRequest::from_text("Noto Sans", "A"));
        assert!(
            HtmlTextMeasurementToken::for_selected_database("Noto Sans", &key, &database).is_none()
        );
    });
}

#[test]
fn token_factory_rejects_outside_nested_and_oversize_requests() {
    assert!(HtmlTextMeasurementToken::for_request("A", "Noto Sans").is_none());
    super::super::with_render_scoped_stamp_batch(|| {
        let oversized = "x".repeat(MAX_FAMILY_BYTES + 1);
        assert!(HtmlTextMeasurementToken::for_request("A", &oversized).is_none());
        super::super::with_render_scoped_stamp_batch(|| {
            assert!(HtmlTextMeasurementToken::for_request("A", "Noto Sans").is_none());
        });
    });
}

#[test]
fn remembered_miss_is_returned_without_constructing_a_token() {
    super::super::font::with_validated_stamp_batch(|| {
        let _scope = super::Scope::enter();
        let key = super::DatabaseKey::eager(super::HtmlFontRequest::from_text("Noto Sans", "A"));
        super::remember_active(super::MemoEntry::Miss(key));
        assert!(HtmlTextMeasurementToken::for_request("A", "Noto Sans").is_none());
    });
}

#[cfg(unix)]
#[test]
fn eager_raw_database_memoizes_the_second_selection() -> Result<(), String> {
    let key = super::DatabaseKey::eager(super::HtmlFontRequest::from_text("Noto Sans", "A"));
    let database = super::super::super::font::bundled_font_db();
    let select = || HtmlTextMeasurementToken::for_selected_database("Noto Sans", &key, &database);
    let first_key = super::super::font::with_validated_stamp_batch(|| {
        let _scope = super::Scope::enter();
        let first = select().ok_or("first eager raw database selection failed")?;
        let second = select().ok_or("second eager raw database selection failed")?;
        assert_eq!(first.key(), second.key());
        assert_eq!(
            super::active_memo_result(&key)
                .flatten()
                .map(|token| token.key()),
            Some(first.key())
        );
        Ok::<_, String>(first.key())
    })?;
    super::super::font::with_validated_stamp_batch(|| {
        let _scope = super::Scope::enter();
        assert!(super::active_memo_result(&key).is_none());
        assert_eq!(select().map(|token| token.key()).as_ref(), Some(&first_key));
    });
    Ok(())
}

#[test]
fn eager_raw_database_remembers_an_unusable_selection() -> Result<(), String> {
    let database = oversized_database()?;
    super::super::font::with_validated_stamp_batch(|| {
        let _scope = super::Scope::enter();
        let key = super::DatabaseKey::eager(super::HtmlFontRequest::from_text("Noto Sans", "A"));
        assert!(
            HtmlTextMeasurementToken::for_selected_database("Noto Sans", &key, &database).is_none()
        );
        assert!(matches!(super::active_memo_result(&key), Some(None)));
        assert!(
            HtmlTextMeasurementToken::for_selected_database("Noto Sans", &key, &database).is_none()
        );
    });
    Ok(())
}

#[test]
fn render_generations_rejects_more_than_the_face_limit() -> Result<(), String> {
    assert!(super::render_generations(&oversized_database()?).is_none());
    Ok(())
}

pub(super) fn oversized_database() -> Result<std::sync::Arc<resvg::usvg::fontdb::Database>, String>
{
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_data(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf").to_vec());
    let face = database.faces().next().ok_or("real face missing")?.clone();
    for _ in 0..super::MAX_TOKEN_FACES {
        database.push_face_info(face.clone());
    }
    let database = std::sync::Arc::new(database);
    assert_eq!(database.faces().count(), super::MAX_TOKEN_FACES + 1);
    Ok(database)
}

#[test]
fn missing_file_reference_disables_generation_reuse() -> Result<(), String> {
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_data(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf").to_vec());
    let mut face = database.faces().next().ok_or("real face missing")?.clone();
    let missing =
        std::env::temp_dir().join(format!("krr-token-missing-{}.ttf", std::process::id()));
    assert!(!missing.exists());
    face.source = resvg::usvg::fontdb::Source::File(missing);
    let mut database = resvg::usvg::fontdb::Database::new();
    database.push_face_info(face);
    let database = std::sync::Arc::new(database);
    super::super::font::with_validated_stamp_batch(|| {
        assert!(super::render_generations(&database).is_none());
    });
    Ok(())
}

#[cfg(unix)]
#[test]
fn bounded_family_input_rejects_oversized_normalized_token() {
    let family = format!("{}Noto Sans", "font-family=".repeat(300));
    assert!(family.len() <= MAX_FAMILY_BYTES);
    let request = super::HtmlFontRequest::from_text(&family, "A");
    let key = super::DatabaseKey::eager(request.clone());
    assert_eq!(request.families.len(), 300);
    assert_eq!(
        request.families.iter().map(String::len).sum::<usize>(),
        539_400
    );
    super::super::with_render_scoped_stamp_batch(|| {
        let database = super::super::html_font_db_for_text(&family, "A");
        assert!(super::render_generations(&database).is_some());
        assert!(HtmlTextMeasurementToken::for_request("A", &family).is_none());
        assert!(matches!(super::active_memo_result(&key), Some(None)));
    });
}
