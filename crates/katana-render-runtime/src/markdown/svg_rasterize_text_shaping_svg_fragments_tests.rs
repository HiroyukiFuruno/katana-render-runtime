use super::{
    FontShape, SvgTextFragmentLayout, fragment_advances, layout_for_face, layout_in_database,
    matching_font_face, resolved_font_runs, rounded_advances_cover_full,
    shaped_svg_fragment_layout,
};
use resvg::usvg::fontdb::Database;
use std::sync::Arc;

const FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const TEST_FONT_SIZE: f32 = 16.0;
const TEST_FONT_WEIGHT: u16 = 400;
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
fn invalid_fragments_rtl_and_ligatures_are_rejected() -> Result<(), String> {
    for fragments in [
        vec![],
        vec!["".to_owned()],
        vec!["A\0".to_owned()],
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
    assert!(layout(&["f", "fi"])?.is_none());
    assert!(layout(&["e", "\u{301}"])?.is_none());
    assert!(fragment_advances(&["é".to_owned()], &[]).is_none());
    Ok(())
}

#[test]
fn invalid_font_sizes_are_rejected() -> Result<(), String> {
    for size in [0.0, -1.0, f32::NAN, f32::INFINITY] {
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
    let runs = resolved_font_runs(&database, base, "Noto Sans", "A日", TEST_FONT_WEIGHT, false);
    assert_eq!(
        runs.len(),
        2,
        "fixture must resolve distinct physical faces"
    );
    assert_ne!(runs[0].0, runs[1].0);
    let fragments = vec!["A".to_owned(), "日".to_owned()];
    assert!(
        layout_in_database(&database, "A日", &fragments, "Noto Sans", font)
            .map_err(|e| format!("{e:?}"))?
            .is_none()
    );
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
