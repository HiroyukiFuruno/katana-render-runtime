use super::super::{
    FontShape, SvgInkBounds, SvgTextRunMetrics, accumulate_svg_font_run_metrics, cache,
    empty_svg_text_metrics, include_svg_primary_strut, matching_font_face, resolved_font_runs,
};
use resvg::usvg::fontdb::{Database, ID};
use std::sync::Arc;

const FAMILY: &str = "Noto Sans";
const WEIGHT: u16 = 400;
const SIZE: f32 = 16.0;
const LATIN_FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const JAPANESE_FONT: &[u8] = include_bytes!(
    "../../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/MPLUS1p-Regular.ttf"
);

fn fixture() -> Result<(Arc<Database>, ID, FontShape), String> {
    let mut database = Database::new();
    database.load_font_data(LATIN_FONT.to_vec());
    database.load_font_data(JAPANESE_FONT.to_vec());
    let database = Arc::new(database);
    let base = matching_font_face(&database, FAMILY, WEIGHT, false).ok_or("Noto Sans missing")?;
    Ok((database, base, FontShape::new(SIZE, WEIGHT, false)))
}

#[test]
fn japanese_fallback_preserves_primary_font_strut() -> Result<(), String> {
    let (database, base, font) = fixture()?;
    let runs = resolved_font_runs(&database, base, FAMILY, "日", WEIGHT, false);
    assert_eq!(runs.len(), 1);
    assert_ne!(runs[0].0, base);
    let mut result = empty_svg_text_metrics(f32::NEG_INFINITY, f32::NEG_INFINITY);
    let includes =
        accumulate_svg_font_run_metrics(&database, base, FAMILY, "日", font, &mut result)
            .ok_or("fallback metrics missing")?;
    assert!(!includes);
    verify_primary_strut(&database, base, font, includes, &mut result)
}

fn verify_primary_strut(
    database: &Database,
    base: ID,
    font: FontShape,
    includes: bool,
    result: &mut SvgTextRunMetrics,
) -> Result<(), String> {
    let before = (
        result.ascent,
        result.descent,
        result.advance,
        result.ink_bounds,
    );
    let (ascent, descent) =
        super::super::super::harfrust::svg_run_endpoints_with_face(database, base, font)
            .ok_or("primary endpoints missing")?;
    include_svg_primary_strut(database, base, font, includes, result)
        .ok_or("primary strut missing")?;
    assert_eq!(result.ascent, before.0.max(ascent));
    assert_eq!(result.descent, before.1.max(descent));
    assert_eq!((result.advance, result.ink_bounds), (before.2, before.3));
    Ok(())
}

#[test]
fn mixed_fallback_glyph_ink_combines_translated_real_runs() -> Result<(), String> {
    let (database, base, font) = fixture()?;
    let runs = resolved_font_runs(&database, base, FAMILY, "A日B", WEIGHT, false);
    assert_eq!(runs.len(), 3);
    assert_eq!((runs[0].0, runs[2].0), (base, base));
    assert_ne!(runs[1].0, base);
    let expected = translated_run_union(&database, &runs, font)?;
    let mut result = empty_svg_text_metrics(f32::NEG_INFINITY, f32::NEG_INFINITY);
    assert!(
        accumulate_svg_font_run_metrics(&database, base, FAMILY, "A日B", font, &mut result,)
            .ok_or("mixed metrics missing")?
    );
    assert_eq!(result.ink_bounds, Some(expected));
    Ok(())
}

#[test]
fn trailing_primary_space_keeps_fallback_ink_bounds() -> Result<(), String> {
    let (database, base, font) = fixture()?;
    let runs = resolved_font_runs(&database, base, FAMILY, "日 ", WEIGHT, false);
    assert_eq!(runs.len(), 2);
    assert_ne!(runs[0].0, base);
    assert_eq!((runs[1].0, runs[1].1.as_str()), (base, " "));
    let first = cache::cached_svg_run(&database, runs[0].0, &runs[0].1, font)
        .ok_or("fallback metrics missing")?;
    let space = cache::cached_svg_run(&database, base, " ", font).ok_or("space metrics missing")?;
    assert!(first.ink_bounds.is_some());
    assert!(space.ink_bounds.is_none() && space.advance > 0.0);
    let mut result = empty_svg_text_metrics(f32::NEG_INFINITY, f32::NEG_INFINITY);
    assert!(
        accumulate_svg_font_run_metrics(&database, base, FAMILY, "日 ", font, &mut result,)
            .ok_or("mixed metrics missing")?
    );
    assert_eq!(result.ink_bounds, first.ink_bounds);
    assert_eq!(result.advance, first.advance + space.advance);
    Ok(())
}

fn translated_run_union(
    database: &Arc<Database>,
    runs: &[(ID, String)],
    font: FontShape,
) -> Result<SvgInkBounds, String> {
    let mut advance = 0.0_f64;
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for (id, text) in runs {
        let metrics: SvgTextRunMetrics =
            cache::cached_svg_run(database, *id, text, font).ok_or("run metrics missing")?;
        let [left, top, right, bottom] = metrics.ink_bounds.ok_or("run ink missing")?;
        let [previous_left, previous_top, previous_right, previous_bottom] = bounds;
        bounds = [
            previous_left.min(left + advance),
            previous_top.min(top),
            previous_right.max(right + advance),
            previous_bottom.max(bottom),
        ];
        advance += f64::from(metrics.advance);
    }
    Ok(bounds)
}
