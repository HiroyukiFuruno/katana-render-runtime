use super::{Prepared, PreparedSpan, paint};
use resvg::usvg::{self, Tree};
use std::sync::Arc;

#[path = "svg_rasterize_html_glyph_anchor_fragment_cases.rs"]
mod cases;
use cases::{ALIGNMENT_NEGATIVE_CASES, AlignmentNegativeCase, WORD_TSPAN_VARIANTS};

const FONT_FAMILY: &str = "Arial";
const SAMPLE: &str = "Internet Banking System";
const FONT_SIZE: f32 = 14.0;
const CSS_SCALE: f32 = 0.999_994_34;
const ANCHOR_X: &str = "120";
const BASELINE_Y: &str = "32";
const POSITION_TOLERANCE: f32 = 1.0 / 64.0;
const UNSPLIT: &str = "Internet Banking System";

#[test]
fn uniform_word_tspans_match_unsplit_arial_after_font_size_correction() -> Result<(), String> {
    let database = html_database();
    let (unsplit_tree, unsplit_spans) = prepared_fixture(&database, "", UNSPLIT)?;
    assert_real_arial(&unsplit_tree, &unsplit_spans)?;
    assert!(unsplit_spans[0].x_alignment.is_some());
    for split in WORD_TSPAN_VARIANTS {
        assert_split_matches_unsplit(&database, &unsplit_spans, split)?;
    }
    Ok(())
}

#[test]
fn multi_fragment_anchor_guard_keeps_unsupported_cases_on_fallback_path() -> Result<(), String> {
    let database = html_database();
    for case in ALIGNMENT_NEGATIVE_CASES {
        assert_alignment_negative(&database, case)?;
    }
    Ok(())
}

fn html_database() -> Arc<usvg::fontdb::Database> {
    super::super::font::html_font_db_for_text(FONT_FAMILY, SAMPLE)
}

fn assert_split_matches_unsplit(
    database: &Arc<usvg::fontdb::Database>,
    unsplit: &[PreparedSpan],
    split_content: &str,
) -> Result<(), String> {
    let (tree, prepared) =
        cases::parse_prepare_fixture(database, "", split_content, "preserve", "", true)?;
    let Prepared::Ready(split) = prepared else {
        return Err("anchor fragment fixture was not prepared".to_owned());
    };
    assert_real_arial(&tree, &split)?;
    assert_multi_fragment_alignment(&split);
    assert_matching_glyphs(unsplit, &split);
    compare_corrected_positions(unsplit, &split);
    Ok(())
}

fn prepared_fixture(
    database: &Arc<usvg::fontdb::Database>,
    attributes: &str,
    content: &str,
) -> Result<(Tree, Vec<PreparedSpan>), String> {
    let (tree, prepared) =
        cases::parse_prepare_fixture(database, attributes, content, "preserve", "", false)?;
    let Prepared::Ready(spans) = prepared else {
        return Err("anchor fragment fixture was not prepared".to_owned());
    };
    Ok((tree, spans))
}

fn fixture_source(attributes: &str, content: &str, xml_space: &str, extra_root: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" xml:space="{xml_space}" x="{ANCHOR_X}" y="{BASELINE_Y}" transform="scale({CSS_SCALE})" font-family="{FONT_FAMILY}" font-size="{FONT_SIZE}" text-anchor="middle" {attributes}>{content}</text>{extra_root}</svg>"#
    )
}

fn assert_real_arial(tree: &Tree, spans: &[PreparedSpan]) -> Result<(), String> {
    for span in spans {
        let face = tree
            .fontdb()
            .face(span.font)
            .ok_or("prepared Arial face missing")?;
        if !face
            .families
            .iter()
            .any(|(family, _)| family.eq_ignore_ascii_case(FONT_FAMILY))
        {
            return Err("fixture resolved to a non-Arial font face".to_owned());
        }
    }
    Ok(())
}

fn assert_multi_fragment_alignment(spans: &[PreparedSpan]) {
    assert!(spans.len() >= 2);
    assert!(spans.iter().all(|span| {
        span.x_alignment.is_some_and(|alignment| {
            alignment.layout.is_none() && alignment.origin_snap_css_scale.is_none()
        })
    }));
}

fn assert_matching_glyphs(expected: &[PreparedSpan], actual: &[PreparedSpan]) {
    let expected_ids = expected
        .iter()
        .flat_map(|span| span.glyph_ids.iter().copied());
    let actual_ids = actual
        .iter()
        .flat_map(|span| span.glyph_ids.iter().copied());
    assert_eq!(
        expected_ids.collect::<Vec<_>>(),
        actual_ids.collect::<Vec<_>>()
    );
}

fn compare_corrected_positions(expected: &[PreparedSpan], actual: &[PreparedSpan]) {
    let expected = corrected_positions(expected);
    let actual = corrected_positions(actual);
    assert_eq!(expected.len(), actual.len());
    for (expected, actual) in expected.iter().zip(actual) {
        assert_eq!(expected.0, actual.0);
        assert!((expected.1 - actual.1).abs() <= POSITION_TOLERANCE);
        assert!((expected.2 - actual.2).abs() <= POSITION_TOLERANCE);
    }
}

fn corrected_positions(spans: &[PreparedSpan]) -> Vec<(u16, f32, f32)> {
    spans
        .iter()
        .flat_map(|span| {
            span.glyphs
                .iter()
                .zip(&span.glyph_ids)
                .map(|(glyph, glyph_id)| {
                    let transform = paint::normalized_transform(span, glyph, CSS_SCALE);
                    (*glyph_id, transform.tx, transform.ty)
                })
        })
        .collect()
}

fn assert_alignment_negative(
    database: &Arc<usvg::fontdb::Database>,
    case: &AlignmentNegativeCase,
) -> Result<(), String> {
    let (_, prepared) = cases::parse_prepare_fixture(
        database,
        case.attributes,
        case.content,
        case.xml_space,
        case.extra_root,
        true,
    )?;
    assert_alignment_not_applied(case, prepared)
}

fn assert_alignment_not_applied(
    case: &AlignmentNegativeCase,
    prepared: Prepared,
) -> Result<(), String> {
    match prepared {
        Prepared::Ready(spans) if case.ready_for_paint => {
            if spans.iter().any(|span| span.x_alignment.is_some()) {
                return Err(format!("{} fixture received alignment", case.name));
            }
            Ok(())
        }
        Prepared::Unsupported if !case.ready_for_paint => Ok(()),
        Prepared::Ready(_) => Err(format!("{} fixture did not use fallback", case.name)),
        Prepared::Unsupported => Err(format!("{} fixture unexpectedly unsupported", case.name)),
    }
}
