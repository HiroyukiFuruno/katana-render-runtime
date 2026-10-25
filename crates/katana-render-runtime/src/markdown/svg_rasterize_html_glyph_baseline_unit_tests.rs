use super::{Policy, PolicyKind, glyph_shift, uniform_policy};
use crate::markdown::svg_rasterize::{
    font::html_font_db_for_text, html_glyph::HtmlGlyphRasterizer,
};
use resvg::usvg::{Node, Options, Tree};

const OBSERVED_FONT_SIZE: f32 = 16.0;
#[cfg(target_os = "macos")]
const PAINT_ORIGIN_TOLERANCE: f32 = 0.0001;

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
                primary_shift: None,
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
