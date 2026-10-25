use super::super::{Prepared, paint};
use crate::markdown::svg_rasterize::{
    font::html_rasterizer_options, html_glyph::HtmlGlyphRasterizer,
};
use resvg::usvg::{Node, Tree};
#[path = "svg_rasterize_html_glyph_baseline_origin_cases_tests.rs"]
mod cases;
use cases::{BaselineOriginCase, MIXED_BASELINE_ORIGINS};

const CHROME_BASELINE_CASE_COUNT: usize = 15;
const CSS_SCALE_COLUMN_COUNT: usize = 3;
const CSS_SCALE_ONE_COLUMN: usize = 0;
const CSS_SCALE_THREE_QUARTER_COLUMN: usize = 1;
const CSS_SCALE_TWO_COLUMN: usize = 2;
const PAINT_ORIGIN_TOLERANCE: f32 = 0.0001;
const BASELINE_CONTROL_GLYPH_COUNT: usize = 2;

const CHROME_BASELINE_ORIGINS: [(&str, [f32; 3]); CHROME_BASELINE_CASE_COUNT] = [
    ("auto-11", [40.0, 30.0, 80.0]),
    ("mathematical-11", [45.0, 34.0, 90.0]),
    ("central-11", [44.0, 33.0, 88.0]),
    ("auto-12", [80.0, 60.0, 160.0]),
    ("mathematical-12", [86.0, 64.0, 171.0]),
    ("central-12", [84.0, 63.0, 168.0]),
    ("auto-13", [120.0, 90.0, 240.0]),
    ("mathematical-13", [126.0, 95.0, 252.0]),
    ("central-13", [125.0, 94.0, 249.0]),
    ("auto-14", [160.0, 120.0, 320.0]),
    ("mathematical-14", [167.0, 125.0, 333.0]),
    ("central-14", [165.0, 124.0, 330.0]),
    ("auto-15", [200.0, 150.0, 400.0]),
    ("mathematical-15", [207.0, 156.0, 414.0]),
    ("central-15", [206.0, 154.0, 411.0]),
];
const CHROME_BASELINE_SCALES: [(&str, f32, usize); CSS_SCALE_COLUMN_COUNT] = [
    ("1", 1.0, CSS_SCALE_ONE_COLUMN),
    ("0.75", 0.75, CSS_SCALE_THREE_QUARTER_COLUMN),
    ("2", 2.0, CSS_SCALE_TWO_COLUMN),
];
#[test]
fn paint_origin_y_matches_chrome_layout_units_for_baseline_controls() -> Result<(), String> {
    let source = include_str!("../../tests/fixtures/svg/baseline-pixel-snap.svg");
    let mut mismatches = Vec::new();
    for (scale, raster_scale, column) in CHROME_BASELINE_SCALES {
        let cases = CHROME_BASELINE_ORIGINS.map(|(id, origins)| BaselineOriginCase {
            id,
            expected_y: origins[column],
            glyph_count: BASELINE_CONTROL_GLYPH_COUNT,
        });
        mismatches.extend(origin_case_mismatches(source, scale, raster_scale, &cases)?);
    }
    assert!(
        mismatches.is_empty(),
        "baseline origin mismatches: {mismatches:#?}"
    );
    Ok(())
}

#[test]
fn mixed_fallback_baseline_origins_match_chrome_controls() -> Result<(), String> {
    assert_origin_fixture_matches(
        include_str!("../../tests/fixtures/svg/baseline-mixed-fallback.svg"),
        &MIXED_BASELINE_ORIGINS,
        "mixed",
    )
}

fn assert_origin_fixture_matches(
    source: &str,
    cases: &[BaselineOriginCase],
    label: &str,
) -> Result<(), String> {
    let mismatches = origin_case_mismatches(source, "1", 1.0, cases)?;
    assert!(
        mismatches.is_empty(),
        "{label} baseline mismatches: {mismatches:#?}"
    );
    Ok(())
}

fn origin_case_mismatches(
    source: &str,
    scale: &str,
    raster_scale: f32,
    cases: &[BaselineOriginCase],
) -> Result<Vec<String>, String> {
    let source = scale_fixture(source, scale)?;
    let options = html_rasterizer_options(&source);
    let tree = Tree::from_str(&source, &options).map_err(|error| error.to_string())?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    Ok(cases
        .iter()
        .filter_map(|case| check_paint_origin(&tree, &rasterizer, case, raster_scale).err())
        .collect())
}

fn scale_fixture(source: &str, scale: &str) -> Result<String, String> {
    let (root, body) = source.split_once('>').ok_or("fixture root missing")?;
    let body = body
        .strip_suffix("</svg>")
        .ok_or("fixture closing tag missing")?;
    Ok(format!(
        "{root}><g transform=\"scale({scale})\">{body}</g></svg>"
    ))
}

fn check_paint_origin(
    tree: &Tree,
    rasterizer: &HtmlGlyphRasterizer<'_>,
    case: &BaselineOriginCase,
    raster_scale: f32,
) -> Result<(), String> {
    let Some(Node::Text(text)) = tree.node_by_id(case.id) else {
        return Err(format!("{}: text node missing", case.id));
    };
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err(format!("{}: text unsupported", case.id));
    };
    collect_origin_mismatches(&spans, case, raster_scale)
}

fn collect_origin_mismatches(
    spans: &[super::super::PreparedSpan],
    case: &BaselineOriginCase,
    raster_scale: f32,
) -> Result<(), String> {
    let actual_count = spans.iter().map(|span| span.glyphs.len()).sum::<usize>();
    let mut mismatches = Vec::new();
    for span in spans {
        for glyph in &span.glyphs {
            let actual_y = paint::normalized_transform(span, glyph, raster_scale).ty;
            if (actual_y - case.expected_y).abs() > PAINT_ORIGIN_TOLERANCE {
                mismatches.push(format!("glyph Y {actual_y}, expected {}", case.expected_y));
            }
        }
    }
    if actual_count != case.glyph_count {
        mismatches.push(format!(
            "glyph count {actual_count}, expected {}",
            case.glyph_count
        ));
    }
    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(format!("{}: {mismatches:?}", case.id))
    }
}
