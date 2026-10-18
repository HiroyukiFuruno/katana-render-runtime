use crate::markdown::svg_rasterize::{SvgRasterizeOps, font, text_shaping};
use font::{DatabaseKey, DiscoveryMode, HtmlFontRequest};
use resvg::usvg::fontdb::Database;
use std::sync::Arc;

const DUPLICATE_FACE_COUNT: usize = 512;
const OVERSIZED_REQUEST_FAMILY_COUNT: usize = 3_000;

const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="40"><text x="4" y="26" font-family="Noto Sans" font-size="18">ABC</text></svg>"#;

fn real_database() -> Result<Database, String> {
    let mut database = Database::new();
    database.load_font_data(font::BUNDLED_SANS_SERIF_FONT.to_vec());
    assert_eq!(database.faces().count(), 1);
    let face = database.faces().next().ok_or("bundled Noto face missing")?;
    assert_eq!(
        database.with_face_data(face.id, |bytes, _| bytes == font::BUNDLED_SANS_SERIF_FONT),
        Some(true)
    );
    Ok(database)
}

fn key(discovery: DiscoveryMode, family: &str) -> DatabaseKey {
    DatabaseKey {
        request: HtmlFontRequest::from_text(family, "ABC"),
        discovery,
    }
}

fn oversized_database() -> Result<Arc<Database>, String> {
    let mut database = real_database()?;
    let face = database
        .faces()
        .next()
        .ok_or("bundled Noto face missing")?
        .clone();
    for _ in 0..DUPLICATE_FACE_COUNT {
        database.push_face_info(face.clone());
    }
    assert_eq!(database.faces().count(), 513);
    Ok(Arc::new(database))
}

fn assert_memoized_miss(value: Option<Option<text_shaping::HtmlTextMeasurementToken>>) {
    assert!(matches!(value, Some(None)));
}

#[test]
fn real_face_limit_records_distinct_known_and_eager_mode_misses() -> Result<(), String> {
    let database = oversized_database()?;
    let known = key(DiscoveryMode::KnownOnly, "Noto Sans");
    let eager = key(DiscoveryMode::Eager, "Noto Sans");
    let memo = text_shaping::with_selection_for_coverage(|select, memo| {
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
fn real_database_byte_budget_rejection_is_remembered_by_mode() -> Result<(), String> {
    let database = Arc::new(real_database()?);
    let oversized = oversized_request();
    text_shaping::with_selection_for_coverage(|select, memo| {
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
    Ok(())
}

#[test]
fn real_mode_token_hit_survives_nested_scope_and_restores_outer_memo() -> Result<(), String> {
    let database = Arc::new(real_database()?);
    let known = key(DiscoveryMode::KnownOnly, "Noto Sans");
    text_shaping::with_selection_for_coverage(|select, memo| -> Result<(), String> {
        let token =
            select(known.clone(), Arc::clone(&database)).ok_or("real bounded token missing")?;
        assert!(token.is_usable());
        assert_eq!(
            memo(&known).flatten().map(|value| value.key()),
            Some(token.key())
        );
        text_shaping::with_selection_for_coverage(|_, nested_memo| {
            assert!(nested_memo(&known).is_none());
        });
        assert_eq!(
            memo(&known).flatten().map(|value| value.key()),
            Some(token.key())
        );
        assert!(token.is_usable());
        Ok(())
    })?;
    Ok(())
}

fn assert_same_real_pixels(tree: &resvg::usvg::Tree) -> Result<(), String> {
    let actual = SvgRasterizeOps::render_tree(tree, 1.0).map_err(|error| error.to_string())?;
    let eager = super::super::html_rasterizer_options(SVG);
    let expected = SvgRasterizeOps::rasterize_with_options(SVG, 1.0, &eager)
        .map_err(|error| error.to_string())?;
    assert_eq!((actual.width, actual.height), (120, 40));
    assert!(actual.rgba.chunks_exact(4).any(|pixel| pixel[3] != 0));
    assert_eq!(
        (actual.width, actual.height, actual.rgba),
        (expected.width, expected.height, expected.rgba)
    );
    Ok(())
}

#[test]
fn inactive_diagnostic_preparation_uses_real_proof_tree_and_pixels() -> Result<(), String> {
    assert!(!super::super::lazy_diagnostic::active());
    let (options, valid, selected, context) =
        super::diagnostic::prepare(SVG, SVG).ok_or("real known preparation unavailable")?;
    assert!(context.is_none());
    assert!(
        options
            .fontdb
            .faces()
            .any(|face| face.families.iter().any(|(name, _)| name == "Noto Sans"))
    );
    let tree =
        text_shaping::with_validated_tree_parse(|| resvg::usvg::Tree::from_str(SVG, &options))
            .map_err(|error| error.to_string())?;
    assert!(valid.load(std::sync::atomic::Ordering::Relaxed));
    assert!(selected.load(std::sync::atomic::Ordering::Relaxed));
    assert_same_real_pixels(&tree)?;
    assert!(!super::super::lazy_diagnostic::active());
    Ok(())
}

#[test]
fn real_known_cache_returns_attached_current_proof() -> Result<(), String> {
    let request = HtmlFontRequest::from_text("Noto Sans", "ABC");
    let value =
        super::discovery_cache::known_for(&request).ok_or("real known lookup unavailable")?;
    let proof = value.proof;
    assert!(proof.current(&value.database));
    assert!(!proof.current(&Database::new()));
    assert!(
        value
            .database
            .faces()
            .any(|face| face.families.iter().any(|(name, _)| name == "Noto Sans"))
    );
    Ok(())
}
