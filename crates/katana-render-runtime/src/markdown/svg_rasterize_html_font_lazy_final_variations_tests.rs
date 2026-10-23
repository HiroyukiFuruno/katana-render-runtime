use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
use resvg::usvg::{
    FontOpticalSizing, FontVariation,
    fontdb::{Database, Source},
};
use std::sync::Arc;

#[test]
fn effective_opsz_observation_matches_usvg_version_bound_rules() {
    let requested = vec![FontVariation::new(*b"wght", 700.0)];
    let auto =
        super::variations::add_auto_opsz(requested.clone(), FontOpticalSizing::Auto, 18.0, true);
    assert_eq!(auto.len(), 2);
    assert_eq!(auto[1].tag, *b"opsz");
    assert_eq!(auto[1].value, 18.0);
    assert_eq!(
        super::variations::add_auto_opsz(requested.clone(), FontOpticalSizing::None, 18.0, true),
        requested
    );
    assert_eq!(
        super::variations::add_auto_opsz(requested.clone(), FontOpticalSizing::Auto, 18.0, false),
        requested
    );
    let explicit = vec![
        FontVariation::new(*b"wght", 700.0),
        FontVariation::new(*b"opsz", 12.0),
    ];
    assert_eq!(
        super::variations::add_auto_opsz(explicit.clone(), FontOpticalSizing::Auto, 18.0, true),
        explicit
    );
}

#[test]
fn normalized_observation_ignores_unknown_axes() -> Result<(), String> {
    let mut database = Database::new();
    let shared_font_bytes = Arc::new(FONT_BYTES.to_vec());
    let ids = database.load_font_source(Source::Binary(shared_font_bytes));
    let face_id = *ids.first().ok_or("bundled font did not load")?;
    let unknown = [FontVariation::new(*b"zzzz", 1.0)];
    assert!(super::variations::normalized_coords(&database, face_id, &unknown)?.is_empty());
    Ok(())
}

#[test]
fn observation_inputs_reject_nonfinite_and_mixed_face_sizes() {
    let variation = [FontVariation::new(*b"wght", 700.0)];
    assert!(super::variations::validate_observation_inputs(&variation, &[16.0, 16.0]).is_ok());
    assert!(super::variations::validate_observation_inputs(&variation, &[16.0, 17.0]).is_err());
    assert!(super::variations::validate_observation_inputs(&variation, &[f32::NAN]).is_err());
    let nonfinite = [FontVariation::new(*b"wght", f32::INFINITY)];
    assert!(super::variations::validate_observation_inputs(&nonfinite, &[16.0]).is_err());
}
