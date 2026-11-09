use super::{
    FontShape, SvgTextFragmentLayout, fragment_advances, html_font_db_for_measured_text,
    layout_for_face, layout_in_database, matching_font_face, resolved_font_runs,
    rounded_advances_cover_full, shaped_svg_fragment_layout,
};
use crate::markdown::svg_rasterize::SvgRasterizeError;
use resvg::usvg::fontdb::Database;
use std::sync::Arc;

#[path = "svg_rasterize_text_shaping_svg_fragment_source_tests.rs"]
mod source_tests;

const FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const FRAGMENT_FAMILY: &str = "Noto Sans";
const SVG_TEXT_FRAGMENTS: [&str; 2] = ["A ", "12-3,$%()"];
const TEST_FONT_SIZE: f32 = 16.0;
const TEST_FONT_WEIGHT: u16 = 400;
const FRAGMENT_ADVANCE_TOLERANCE: f32 = 0.0001;
const JAPANESE_FONT: &[u8] = include_bytes!(
    "../../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/MPLUS1p-Regular.ttf"
);

fn layout(parts: &[&str]) -> Result<Option<SvgTextFragmentLayout>, String> {
    let mut database = Database::new();
    database.load_font_data(FONT.to_vec());
    let face = database.faces().next().ok_or("font face missing")?.id;
    let fragments: Vec<String> = parts.iter().map(|part| (*part).to_owned()).collect();
    let text = fragments.concat();
    layout_for_face(
        &database,
        face,
        face,
        &text,
        FontShape::new(TEST_FONT_SIZE, TEST_FONT_WEIGHT, false),
        &fragments,
    )
    .map_err(|error| format!("{error:?}"))
}

#[test]
fn fragment_advances_keep_full_run_kerning_and_scalar_counts() -> Result<(), String> {
    let result = layout(&["A", "V"])?.ok_or("AV layout rejected")?;
    let together = layout(&["AV"])?.ok_or("whole AV rejected")?;
    let separate_a = layout(&["A"])?.ok_or("A layout rejected")?;
    assert_eq!(result.advances.len(), 2);
    assert!((result.advances.iter().sum::<f32>() - together.metrics.advance).abs() < 0.0001);
    assert!(result.advances[0] < separate_a.metrics.advance);
    let unicode = layout(&["é", " A"])?.ok_or("multibyte layout rejected")?;
    assert_eq!(unicode.advances.len(), 2);
    assert!((unicode.advances.iter().sum::<f32>() - unicode.metrics.advance).abs() < 0.0001);
    Ok(())
}

#[test]
fn high_level_fragment_layout_preserves_selected_face_and_total_advance() -> Result<(), String> {
    let fragments = svg_text_fragments();
    let text = fragments.concat();
    assert_selected_face_for_fragments(&fragments, &text)?;
    assert_high_level_fragment_contract(&fragments)
}

fn svg_text_fragments() -> Vec<String> {
    SVG_TEXT_FRAGMENTS
        .iter()
        .map(|fragment| (*fragment).to_owned())
        .collect()
}

fn assert_selected_face_for_fragments(fragments: &[String], text: &str) -> Result<(), String> {
    let font = svg_font_shape();
    let database = html_font_db_for_measured_text(FRAGMENT_FAMILY, text, font.weight, font.italic);
    let base = matching_font_face(&database, FRAGMENT_FAMILY, font.weight, font.italic)
        .ok_or("Noto Sans face missing")?;
    assert_face_metadata(&database, base)?;
    assert_fragment_runs(&database, base, fragments, text, font);
    Ok(())
}

fn assert_face_metadata(
    database: &Database,
    face_id: resvg::usvg::fontdb::ID,
) -> Result<(), String> {
    let face = database
        .face(face_id)
        .ok_or("selected Noto Sans face disappeared")?;
    assert!(
        face.families
            .iter()
            .any(|(family, _)| family.eq_ignore_ascii_case(FRAGMENT_FAMILY))
    );
    assert_eq!(face.weight.0, TEST_FONT_WEIGHT);
    assert_eq!(face.style, resvg::usvg::fontdb::Style::Normal);
    Ok(())
}

fn assert_fragment_runs(
    database: &Arc<Database>,
    face_id: resvg::usvg::fontdb::ID,
    fragments: &[String],
    text: &str,
    font: FontShape,
) {
    for input in std::iter::once(text).chain(fragments.iter().map(String::as_str)) {
        let runs = resolved_font_runs(
            database,
            face_id,
            FRAGMENT_FAMILY,
            input,
            font.weight,
            font.italic,
        );
        assert_eq!(runs.as_slice(), &[(face_id, input.to_owned())]);
    }
}

fn assert_high_level_fragment_contract(fragments: &[String]) -> Result<(), String> {
    let font = svg_font_shape();
    let layout = shaped_svg_fragment_layout(
        fragments,
        FRAGMENT_FAMILY,
        font.size,
        font.weight,
        font.italic,
    )
    .map_err(|error| format!("{error:?}"))?
    .ok_or("high-level Noto Sans fragment layout rejected")?;
    assert_eq!(layout.advances.len(), fragments.len());
    assert!(layout.metrics.advance.is_finite() && layout.metrics.advance > 0.0);
    assert!(
        layout
            .advances
            .iter()
            .all(|advance| advance.is_finite() && *advance > 0.0)
    );
    assert!(
        (layout.advances.iter().sum::<f32>() - layout.metrics.advance).abs()
            <= FRAGMENT_ADVANCE_TOLERANCE
    );
    Ok(())
}

fn svg_font_shape() -> FontShape {
    FontShape::new(TEST_FONT_SIZE, TEST_FONT_WEIGHT, false)
}

#[test]
fn missing_font_face_is_a_rasterization_error() {
    let database = Arc::new(Database::new());
    let font = FontShape::new(TEST_FONT_SIZE, TEST_FONT_WEIGHT, false);
    let fragments = vec!["A".to_owned()];
    let result = layout_in_database(&database, "A", &fragments, FRAGMENT_FAMILY, font);
    assert!(matches!(
        result,
        Err(SvgRasterizeError::RasterizeFailed(message))
            if message == "SVG font face unavailable"
    ));
}

#[test]
fn invalid_fragments_rtl_and_ligatures_are_rejected() -> Result<(), String> {
    for fragments in [
        vec![],
        vec!["".to_owned()],
        vec!["A\0".to_owned()],
        vec!["A\u{0001}".to_owned()],
        vec!["A".to_owned(), "אב".to_owned()],
        vec!["A\u{200b}".to_owned()],
        vec!["A\u{2066}B\u{2069}".to_owned()],
        vec!["A\u{200d}".to_owned()],
    ] {
        assert!(
            shaped_svg_fragment_layout(
                &fragments,
                "Noto Sans",
                TEST_FONT_SIZE,
                TEST_FONT_WEIGHT,
                false
            )
            .map_err(|e| format!("{e:?}"))?
            .is_none()
        );
    }
    assert!(layout(&["A", "אב"])?.is_none());
    assert!(layout(&["f", "fi"])?.is_none());
    assert!(layout(&["e", "\u{301}"])?.is_none());
    assert!(fragment_advances(&["é".to_owned()], &[]).is_none());
    Ok(())
}

#[test]
fn invalid_font_sizes_are_rejected() -> Result<(), String> {
    for size in [
        0.0,
        -1.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MIN_POSITIVE,
        f32::MAX,
    ] {
        assert!(
            shaped_svg_fragment_layout(
                &["A".to_owned()],
                "Noto Sans",
                size,
                TEST_FONT_WEIGHT,
                false
            )
            .map_err(|e| format!("{e:?}"))?
            .is_none()
        );
    }
    Ok(())
}

#[test]
fn real_fallback_faces_are_not_grouped() -> Result<(), String> {
    let mut database = Database::new();
    database.load_font_data(FONT.to_vec());
    database.load_font_data(JAPANESE_FONT.to_vec());
    let database = Arc::new(database);
    let font = FontShape::new(TEST_FONT_SIZE, TEST_FONT_WEIGHT, false);
    let base = matching_font_face(&database, "Noto Sans", TEST_FONT_WEIGHT, false)
        .ok_or("Noto face missing")?;
    for parts in [&["A", "日"][..], &["日", "A"][..], &["A", "日", "B"][..]] {
        let fragments: Vec<String> = parts.iter().map(|part| (*part).to_owned()).collect();
        let text = fragments.concat();
        let runs = resolved_font_runs(&database, base, "Noto Sans", &text, TEST_FONT_WEIGHT, false);
        assert_eq!(runs.len(), parts.len(), "fallback runs for {text}");
        assert_eq!(
            runs.iter().map(|(_, run)| run.as_str()).collect::<String>(),
            text
        );
        for pair in runs.windows(2) {
            assert_ne!(pair[0].0, pair[1].0, "adjacent runs share a face: {text}");
        }
        assert!(
            layout_in_database(&database, &text, &fragments, "Noto Sans", font)
                .map_err(|e| format!("{e:?}"))?
                .is_none()
        );
    }
    Ok(())
}

#[test]
fn rounded_fragment_width_cannot_underrun_full_advance() {
    const FULL_ADVANCE_BITS: u32 = 0x4480_03e8;
    const SMALL_FRAGMENT_BITS: u32 = 0x3d9c_4000;
    let full = f32::from_bits(FULL_ADVANCE_BITS);
    let fragments = [1024.0_f32, f32::from_bits(SMALL_FRAGMENT_BITS)];
    assert!(!rounded_advances_cover_full(&fragments, full));
    assert!(rounded_advances_cover_full(&[1.001, 2.001], 3.002));
}

#[test]
fn visible_overhang_and_missing_glyph_are_ineligible() -> Result<(), String> {
    assert!(layout(&["j"])?.is_none());
    assert!(layout(&["\u{10ffff}"])?.is_none());
    Ok(())
}
