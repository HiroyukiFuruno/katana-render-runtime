use crate::markdown::svg_rasterize::SvgRasterizeOps;
use crate::markdown::svg_rasterize::{font, text_shaping};
use font::HtmlFontRequest;
use resvg::usvg::fontdb::Database;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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

#[test]
fn default_fallback_invalidates_fast_path_for_candidate_and_no_match() -> Result<(), String> {
    let mut database = real_database()?;
    database.load_font_data(font::BUNDLED_SANS_SERIF_FONT.to_vec());
    let ids: Vec<_> = database.faces().map(|face| face.id).collect();
    let [base, candidate] = ids.as_slice() else {
        return Err("expected two bundled font faces".into());
    };
    let mut database = Arc::new(database);
    let valid = Arc::new(AtomicBool::new(true));
    let mut options = resvg::usvg::Options::default();
    super::install_fallback(&mut options, Arc::clone(&valid));
    let stock = resvg::usvg::FontResolver::default_fallback_selector();

    for (excluded, expected) in [
        (vec![*base], Some(*candidate)),
        (vec![*base, *candidate], None),
    ] {
        assert_eq!(stock('A', &excluded, &mut database), expected);
        valid.store(true, Ordering::Relaxed);
        assert_eq!(
            (options.font_resolver.select_fallback)('A', &excluded, &mut database),
            expected
        );
        assert!(!valid.load(Ordering::Relaxed));
    }
    Ok(())
}

fn assert_same_real_pixels(tree: &resvg::usvg::Tree) -> Result<(), String> {
    let compatible = SvgRasterizeOps::preprocess_for_rasterizer(SVG);
    let actual =
        SvgRasterizeOps::render_tree(tree, 1.0, &compatible).map_err(|error| error.to_string())?;
    let eager = super::super::html_rasterizer_options(SVG);
    let expected = SvgRasterizeOps::rasterize_with_options(SVG, 1.0, &eager)
        .map_err(|error| error.to_string())?;
    assert_eq!((actual.width, actual.height), (120, 40));
    assert!(
        actual
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] != 0)
    );
    assert_eq!(
        (actual.width, actual.height, actual.rgba),
        (expected.width, expected.height, expected.rgba)
    );
    Ok(())
}

#[test]
fn inactive_diagnostic_preparation_uses_real_proof_tree_and_pixels() -> Result<(), String> {
    assert!(!super::super::lazy_diagnostic::active());
    let document = resvg::usvg::roxmltree::Document::parse(SVG);
    let (options, valid, selected, context) =
        super::diagnostic::prepare(&document, SVG).ok_or("real known preparation unavailable")?;
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
