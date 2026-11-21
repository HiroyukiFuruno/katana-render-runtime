use super::{
    Database, DatabaseKey, HtmlFontRequest, HtmlTextMeasurementToken, Scope, active_memo_result,
    tests::oversized_database, token_for_selected,
};
use crate::markdown::svg_rasterize::font::{DiscoveryMode, bundled_font_db};
use std::sync::Arc;

const OVERSIZED_REQUEST_FAMILY_COUNT: usize = 3_000;

fn with_selection<T>(
    mut operation: impl FnMut(
        fn(DatabaseKey, Arc<Database>) -> Option<HtmlTextMeasurementToken>,
        fn(&DatabaseKey) -> Option<Option<HtmlTextMeasurementToken>>,
    ) -> T,
) -> T {
    super::super::font::with_validated_stamp_batch(|| {
        let _scope = Scope::enter();
        operation(token_for_selected, active_memo_result)
    })
}

fn key(discovery: DiscoveryMode, family: &str) -> DatabaseKey {
    DatabaseKey {
        request: HtmlFontRequest::from_text(family, "ABC"),
        discovery,
    }
}

fn assert_memoized_miss(value: Option<Option<HtmlTextMeasurementToken>>) {
    assert!(matches!(value, Some(None)));
}

#[test]
fn real_face_limit_records_distinct_known_and_eager_mode_misses() -> Result<(), String> {
    let database = oversized_database()?;
    let known = key(DiscoveryMode::KnownOnly, "Noto Sans");
    let eager = key(DiscoveryMode::Eager, "Noto Sans");
    let memo = with_selection(|select, memo| {
        assert!(select(known.clone(), Arc::clone(&database)).is_none());
        assert_memoized_miss(memo(&known));
        assert!(memo(&eager).is_none());
        assert!(select(eager.clone(), Arc::clone(&database)).is_none());
        assert_memoized_miss(memo(&eager));
        assert_memoized_miss(memo(&known));
        assert!(memo(&key(DiscoveryMode::KnownOnly, "other request")).is_none());
        memo
    });
    assert!(memo(&known).is_none());
    assert!(memo(&eager).is_none());
    Ok(())
}

fn oversized_request() -> DatabaseKey {
    let family = (0..OVERSIZED_REQUEST_FAMILY_COUNT)
        .map(|index| format!("family-{index:0100}"))
        .collect::<Vec<_>>()
        .join(",");
    let key = key(DiscoveryMode::KnownOnly, &family);
    assert_eq!(key.request.families.len(), OVERSIZED_REQUEST_FAMILY_COUNT);
    assert!(key.request.families.iter().map(String::len).sum::<usize>() > 256 * 1024);
    key
}

#[test]
fn real_database_byte_budget_rejection_is_remembered_by_mode() {
    let database = bundled_font_db();
    let oversized = oversized_request();
    with_selection(|select, memo| {
        assert!(
            select(
                key(DiscoveryMode::KnownOnly, "Noto Sans"),
                Arc::clone(&database)
            )
            .is_some()
        );
        assert!(select(oversized.clone(), Arc::clone(&database)).is_none());
        assert!(matches!(memo(&oversized), Some(None)));
        assert!(
            memo(&key(
                DiscoveryMode::Eager,
                &oversized.request.families.join(",")
            ))
            .is_none()
        );
    });
}

#[test]
fn real_mode_token_hit_survives_nested_scope_and_restores_outer_memo() -> Result<(), String> {
    let database = bundled_font_db();
    let known = key(DiscoveryMode::KnownOnly, "Noto Sans");
    let outer_key = with_selection(|select, memo| {
        let token =
            select(known.clone(), Arc::clone(&database)).ok_or("real bounded token missing")?;
        assert!(token.is_usable());
        assert_eq!(
            memo(&known).flatten().map(|value| value.key()),
            Some(token.key())
        );
        with_selection(|_, nested_memo| {
            assert!(nested_memo(&known).is_none());
        });
        assert_eq!(
            memo(&known).flatten().map(|value| value.key()),
            Some(token.key())
        );
        assert!(token.is_usable());
        Ok::<_, String>(token.key())
    })?;
    with_selection(|select, memo| {
        assert!(memo(&known).is_none());
        let stored = select(known.clone(), Arc::clone(&database));
        assert_eq!(stored.map(|value| value.key()).as_ref(), Some(&outer_key));
    });
    Ok(())
}
