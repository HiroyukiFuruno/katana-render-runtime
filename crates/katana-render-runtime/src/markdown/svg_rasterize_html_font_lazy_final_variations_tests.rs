use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
use resvg::usvg::{
    FontOpticalSizing, FontVariation,
    fontdb::{Database, ID, Source},
};
use std::sync::Arc;

#[cfg(target_os = "macos")]
use skrifa::{FontRef, MetadataProvider, Tag};

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
    assert!(
        super::variations::applied_design_coordinates(&database, face_id, &unknown)?.is_empty()
    );
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

#[cfg(target_os = "macos")]
fn sfns_face() -> Result<(Database, ID), String> {
    let bytes = std::fs::read("/System/Library/Fonts/SFNS.ttf")
        .map_err(|error| format!("read SFNS.ttf: {error}"))?;
    let mut database = Database::new();
    let mut ids = database.load_font_source(Source::Binary(Arc::new(bytes)));
    let face_id = ids
        .drain(..)
        .next()
        .ok_or_else(|| "SFNS.ttf face index 0 missing".to_owned())?;
    Ok((database, face_id))
}

#[cfg(target_os = "macos")]
#[test]
fn sfns_design_coordinates_clamp_and_preserve_defaults() -> Result<(), String> {
    let (database, face_id) = sfns_face()?;
    let axes = sfns_axis_expectations(&database, face_id)?;
    let design = super::variations::applied_design_coordinates(
        &database,
        face_id,
        &[
            FontVariation::new(*b"opsz", axes.opsz_min - 1.0),
            FontVariation::new(*b"wght", axes.wght_max + 1.0),
        ],
    )?;
    assert_sfns_design(&design, &axes);
    assert_sfns_normalized(&database, face_id, &design)?;
    Ok(())
}

#[cfg(target_os = "macos")]
struct SfnsAxisExpectations {
    opsz_min: f32,
    wght_max: f32,
    opsz_index: usize,
    wght_index: usize,
    defaults: Vec<(usize, f32)>,
}

#[cfg(target_os = "macos")]
fn sfns_axis_expectations(
    database: &Database,
    face_id: ID,
) -> Result<SfnsAxisExpectations, String> {
    let values = database
        .with_face_data(face_id, |data, index| {
            let font = FontRef::from_index(data, index).map_err(|error| error.to_string())?;
            let axes = font.axes();
            let opsz = axes
                .get_by_tag(Tag::from_be_bytes(*b"opsz"))
                .ok_or("SFNS opsz axis missing")?;
            let wght = axes
                .get_by_tag(Tag::from_be_bytes(*b"wght"))
                .ok_or("SFNS wght axis missing")?;
            let defaults = axes
                .iter()
                .map(|axis| (axis.index(), axis.default_value()))
                .collect::<Vec<_>>();
            Ok::<_, String>(SfnsAxisExpectations {
                opsz_min: opsz.min_value(),
                wght_max: wght.max_value(),
                opsz_index: opsz.index(),
                wght_index: wght.index(),
                defaults,
            })
        })
        .ok_or_else(|| "SFNS face data missing".to_owned())??;
    Ok(values)
}

#[cfg(target_os = "macos")]
fn assert_sfns_design(design: &[FontVariation], axes: &SfnsAxisExpectations) {
    assert_eq!(design[axes.opsz_index].value, axes.opsz_min);
    assert_eq!(design[axes.wght_index].value, axes.wght_max);
    for (index, default) in &axes.defaults {
        if *index != axes.opsz_index && *index != axes.wght_index {
            assert_eq!(design[*index].value, *default);
        }
    }
}

#[cfg(target_os = "macos")]
fn assert_sfns_normalized(
    database: &Database,
    face_id: ID,
    design: &[FontVariation],
) -> Result<(), String> {
    let normalized = super::variations::normalized_coords(database, face_id, design)?;
    let expected = sfns_normalized_reference(database, face_id, design)?;
    assert_eq!(
        normalized
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn sfns_normalized_reference(
    database: &Database,
    face_id: ID,
    design: &[FontVariation],
) -> Result<Vec<f32>, String> {
    database
        .with_face_data(face_id, |data, index| {
            let font = FontRef::from_index(data, index).map_err(|error| error.to_string())?;
            let settings = design
                .iter()
                .map(|variation| (Tag::from_be_bytes(variation.tag), variation.value));
            Ok::<_, String>(
                font.axes()
                    .location(settings)
                    .coords()
                    .iter()
                    .map(|coord| coord.to_f32())
                    .collect::<Vec<_>>(),
            )
        })
        .ok_or_else(|| "SFNS face data missing".to_owned())?
}

#[cfg(target_os = "macos")]
#[test]
fn sfns_design_coordinates_reject_unknown_axis() -> Result<(), String> {
    let (database, face_id) = sfns_face()?;
    let result = super::variations::applied_design_coordinates(
        &database,
        face_id,
        &[FontVariation::new(*b"zzzz", 1.0)],
    );
    assert!(result.is_err());
    Ok(())
}
