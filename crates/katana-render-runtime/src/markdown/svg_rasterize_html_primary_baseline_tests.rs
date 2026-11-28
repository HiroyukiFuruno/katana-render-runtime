use super::{
    MAX_PRIMARY_BASELINE_ENTRIES, PRIMARY_BASELINE_CACHE, clear_primary_baseline_cache,
    measure_primary_baseline_for_face, primary_baseline,
};
use crate::markdown::svg_rasterize::font_file_fixture::{self, FONT_BYTES};

#[test]
fn primary_baseline_cache_is_bounded_and_released() {
    clear_primary_baseline_cache();
    for height in 1..=MAX_PRIMARY_BASELINE_ENTRIES + 1 {
        assert!(
            primary_baseline("sans-serif", 12.0, 400, false, height as f32)
                .is_some_and(|baseline| baseline.is_finite()),
            "fixture baseline must exist and be finite for height {height}"
        );
    }
    PRIMARY_BASELINE_CACHE.with(|cache| assert_eq!(cache.borrow().len(), 1));
    clear_primary_baseline_cache();
    PRIMARY_BASELINE_CACHE.with(|cache| assert_eq!(cache.borrow().capacity(), 0));
}

#[test]
fn missing_primary_font_does_not_cache_a_baseline_and_recovers() {
    clear_primary_baseline_cache();
    assert_eq!(
        primary_baseline("KRR Font Family Missing 8f0c254a", 12.0, 400, false, 18.0,),
        None
    );
    PRIMARY_BASELINE_CACHE.with(|cache| assert!(cache.borrow().is_empty()));

    assert!(primary_baseline("sans-serif", 12.0, 400, false, 18.0).is_some_and(f32::is_finite));
}

#[test]
fn missing_primary_font_source_returns_no_baseline() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = font_file_fixture::create(directory.path().join("font.ttf"), FONT_BYTES)
        .map_err(|error| error.to_string())?;
    assert!(
        measure_primary_baseline_for_face(&fixture.database, fixture.face_id, 12.0, 18.0).is_some()
    );

    std::fs::remove_file(&fixture.path).map_err(|error| error.to_string())?;
    assert_eq!(
        measure_primary_baseline_for_face(&fixture.database, fixture.face_id, 12.0, 18.0),
        None
    );
    Ok(())
}
