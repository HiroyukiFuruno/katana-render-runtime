use super::{Policy, PolicyKind, glyph_shift, uniform_policy};
use crate::markdown::svg_rasterize::{
    font::html_font_db_for_text, html_glyph::HtmlGlyphRasterizer,
};
use resvg::usvg::{Node, Options, Tree};

const OBSERVED_FONT_SIZE: f32 = 16.0;
#[cfg(target_os = "macos")]
const CHROME_BASELINE_CASE_COUNT: usize = 15;
#[cfg(target_os = "macos")]
const CSS_SCALE_COLUMN_COUNT: usize = 3;
#[cfg(target_os = "macos")]
const CSS_SCALE_ONE_COLUMN: usize = 0;
#[cfg(target_os = "macos")]
const CSS_SCALE_THREE_QUARTER_COLUMN: usize = 1;
#[cfg(target_os = "macos")]
const CSS_SCALE_TWO_COLUMN: usize = 2;
#[cfg(target_os = "macos")]
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
#[cfg(target_os = "macos")]
const CHROME_BASELINE_SCALES: [(&str, f32, usize); CSS_SCALE_COLUMN_COUNT] = [
    ("1", 1.0, CSS_SCALE_ONE_COLUMN),
    ("0.75", 0.75, CSS_SCALE_THREE_QUARTER_COLUMN),
    ("2", 2.0, CSS_SCALE_TWO_COLUMN),
];
#[cfg(target_os = "macos")]
const PAINT_ORIGIN_TOLERANCE: f32 = 0.0001;

#[cfg(target_os = "macos")]
#[test]
fn paint_origin_y_matches_chrome_layout_units_for_baseline_controls() -> Result<(), String> {
    let source = include_str!("../../tests/fixtures/svg/baseline-pixel-snap.svg");
    let mut mismatches = Vec::new();
    for (scale, raster_scale, column) in CHROME_BASELINE_SCALES {
        let scaled_source = scale_fixture(source, scale)?;
        let database = html_font_db_for_text("Helvetica", "Ag");
        let tree = Tree::from_str(
            &scaled_source,
            &Options {
                fontdb: database,
                ..Options::default()
            },
        )
        .map_err(|error| error.to_string())?;
        let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
        for (id, origins) in CHROME_BASELINE_ORIGINS {
            if let Err(error) =
                check_paint_origin(&tree, &rasterizer, id, origins[column], raster_scale)
            {
                mismatches.push(error);
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "baseline origin mismatches: {mismatches:#?}"
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn scale_fixture(source: &str, scale: &str) -> Result<String, String> {
    let (root, body) = source.split_once('>').ok_or("fixture root missing")?;
    let body = body
        .strip_suffix("</svg>")
        .ok_or("fixture closing tag missing")?;
    Ok(format!(
        "{root}><g transform=\"scale({scale})\">{body}</g></svg>"
    ))
}

#[cfg(target_os = "macos")]
fn check_paint_origin(
    tree: &Tree,
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: &str,
    expected_y: f32,
    raster_scale: f32,
) -> Result<(), String> {
    let Some(Node::Text(text)) = tree.node_by_id(id) else {
        return Err(format!("{id}: text node missing"));
    };
    let super::super::Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err(format!("{id}: text unsupported"));
    };
    let Some(span) = spans.first() else {
        return Err(format!("{id}: prepared span missing"));
    };
    let Some(glyph) = span.glyphs.first() else {
        return Err(format!("{id}: glyph missing"));
    };
    let actual_y = super::super::paint::normalized_transform(span, glyph, raster_scale).ty;
    if (actual_y - expected_y).abs() > PAINT_ORIGIN_TOLERANCE {
        return Err(format!("{id}: actual Y {actual_y}, expected {expected_y}"));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn helvetica_math_and_central_use_blink_postprocessed_metrics() -> Result<(), String> {
    let database = html_font_db_for_text("Helvetica", "A");
    let id = find_helvetica_face(&database)?;
    let rasterizer = HtmlGlyphRasterizer::new(&database);
    let face = rasterizer
        .ensure_face(id)?
        .ok_or("Helvetica face unavailable")?;
    assert_shift(&database, id, &face, PolicyKind::Mathematical, 0.83984375);
    assert_shift(&database, id, &face, PolicyKind::Central, 0.6796875);
    Ok(())
}

#[cfg(target_os = "macos")]
fn assert_shift(
    database: &resvg::usvg::fontdb::Database,
    id: resvg::usvg::fontdb::ID,
    face: &super::super::CachedFace,
    kind: PolicyKind,
    expected: f32,
) {
    for css_scale in [Some(1.0), None] {
        let actual = glyph_shift(
            database,
            id,
            face,
            Policy {
                kind,
                font_size: OBSERVED_FONT_SIZE,
                css_scale,
            },
        );
        assert!(
            (actual - expected).abs() < PAINT_ORIGIN_TOLERANCE,
            "{kind:?} at {css_scale:?}: actual {actual}, expected {expected}"
        );
    }
}

#[cfg(target_os = "macos")]
fn find_helvetica_face(
    database: &resvg::usvg::fontdb::Database,
) -> Result<resvg::usvg::fontdb::ID, String> {
    database
        .faces()
        .find(|face| {
            face.weight == resvg::usvg::fontdb::Weight::NORMAL
                && face.families.iter().any(|(name, _)| name == "Helvetica")
        })
        .map(|face| face.id)
        .ok_or_else(|| "Helvetica face missing".to_owned())
}

#[test]
fn baseline_policy_is_conservative_for_nonuniform_text() -> Result<(), String> {
    assert!(parse_policy(r#"<text id="run" x="0" y="20">A</text>"#)?.is_none());
    assert!(parse_policy(r#"<text id="run" dominant-baseline="mathematical" x="0" y="20"><tspan>A</tspan></text>"#)?.is_some());
    assert!(parse_policy(r#"<text id="run" dominant-baseline="mathematical" x="0" y="20">A<tspan alignment-baseline="central">B</tspan></text>"#)?.is_none());
    assert!(parse_policy(r#"<text id="run" dominant-baseline="mathematical" x="0" y="20"><tspan font-size="12">A</tspan><tspan font-size="16">B</tspan></text>"#)?.is_none());
    assert!(parse_policy(r#"<text id="run" writing-mode="vertical-rl" dominant-baseline="mathematical" x="0" y="20">A</text>"#)?.is_none());
    assert!(parse_policy(r##"<path id="p" d="M0 0h20"/><text id="run" dominant-baseline="mathematical"><textPath href="#p">A</textPath></text>"##)?.is_none());
    Ok(())
}

#[test]
fn baseline_policy_uses_legacy_metrics_for_nonuniform_or_invalid_scale() -> Result<(), String> {
    let policy = parse_policy(
        r#"<text id="run" transform="scale(2 1)" dominant-baseline="mathematical" x="0" y="20">A</text>"#,
    )?
    .ok_or("nonuniform baseline policy missing")?;
    assert_eq!(policy.css_scale, None);
    assert_eq!(
        super::super::anchor::uniform_css_scale(tiny_skia::Transform::from_row(
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0
        )),
        None
    );
    Ok(())
}

fn parse_policy(body: &str) -> Result<Option<Policy>, String> {
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40" font-family="Helvetica" font-size="16">{body}</svg>"#
    );
    let database = html_font_db_for_text("Helvetica", "AB");
    let tree = Tree::from_str(
        &source,
        &Options {
            fontdb: database,
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("text node missing".to_owned());
    };
    Ok(uniform_policy(text))
}
