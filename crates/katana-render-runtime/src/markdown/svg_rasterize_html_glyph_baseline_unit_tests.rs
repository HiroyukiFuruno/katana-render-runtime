use super::{Policy, PolicyKind, glyph_shift, uniform_policy};
use crate::markdown::svg_rasterize::{
    font::html_font_db_for_text, html_glyph::HtmlGlyphRasterizer,
};
use resvg::usvg::{Node, Options, Tree};

const OBSERVED_FONT_SIZE: f32 = 16.0;
const FIXTURE_FONT_SIZE: f32 = 10.0;
const FIXTURE_UNITS_PER_EM: f32 = 1000.0;
const FIXTURE_ASCENT: f32 = 800.0;
const FIXTURE_DESCENT: f32 = -200.0;
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
    for kind in [PolicyKind::Middle, PolicyKind::AlignmentMiddle] {
        let mut policy = test_policy(kind);
        assert_eq!(glyph_shift(&database, id, &face, policy), 0.0);
        policy.primary_shift = Some(2.5);
        assert_eq!(glyph_shift(&database, id, &face, policy), 2.5);
    }
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
fn middle_policy_requires_source_proof_and_rejects_unsupported_transforms() -> Result<(), String> {
    /* WHY: dyなし・0の追加laneにも原DOMの肯定証明が必要で、modelだけでは認めない。 */
    for body in [
        r#"<text id="run" dominant-baseline="middle" x="0" y="20">AB</text>"#,
        r#"<text id="run" dominant-baseline="middle" dy="0" x="0" y="20">AB</text>"#,
        r#"<text id="run" dominant-baseline="middle" dy="0 1" x="0" y="20">AB</text>"#,
        r#"<text id="run" transform="scale(2 1)" dominant-baseline="middle" dy="1" x="0" y="20">AB</text>"#,
    ] {
        assert!(parse_policy(body)?.is_none());
    }
    Ok(())
}

#[test]
fn middle_policy_preserves_math_and_central_leading_dy_policies() -> Result<(), String> {
    let supported = [
        (
            r#"<text id="run" dominant-baseline="middle" dy="1" x="0" y="20">AB</text>"#,
            PolicyKind::Middle,
        ),
        (
            r#"<text id="run" dominant-baseline="mathematical" dy="1" x="0" y="20">AB</text>"#,
            PolicyKind::Mathematical,
        ),
        (
            r#"<text id="run" dominant-baseline="central" dy="1" x="0" y="20">AB</text>"#,
            PolicyKind::Central,
        ),
    ];
    for (body, expected) in supported {
        assert_eq!(
            parse_policy(body)?.map(|policy| policy.kind),
            Some(expected)
        );
    }
    Ok(())
}

#[test]
fn mathematical_policy_keeps_primary_shift_only_when_font_is_loaded() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = crate::markdown::svg_rasterize::font_file_fixture::create(
        directory.path().join("NotoSans-Regular.ttf"),
        crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES,
    )
    .map_err(|error| error.to_string())?;
    let tree = mathematical_fixture_tree(&fixture.database)?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("mathematical baseline text missing".to_owned());
    };
    let loaded = HtmlGlyphRasterizer::new(&fixture.database);
    let empty_database = resvg::usvg::fontdb::Database::new();
    let unloaded = HtmlGlyphRasterizer::new(&empty_database);
    let loaded_policy = super::resolved_policy(&loaded, text)?
        .ok_or("loaded mathematical baseline policy missing")?;
    let unloaded_policy = super::resolved_policy(&unloaded, text)?
        .ok_or("unloaded mathematical baseline policy missing")?;
    assert_eq!(loaded_policy.kind, PolicyKind::Mathematical);
    assert_eq!(unloaded_policy.kind, PolicyKind::Mathematical);
    assert!(loaded_policy.primary_shift.is_some_and(f32::is_finite));
    assert_eq!(unloaded_policy.primary_shift, None);
    Ok(())
}

fn mathematical_fixture_tree(database: &resvg::usvg::fontdb::Database) -> Result<Tree, String> {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" x="0" y="20" dy="1" font-family="Noto Sans" dominant-baseline="mathematical">AB</text></svg>"#;
    Tree::from_str(
        source,
        &Options {
            fontdb: std::sync::Arc::new(database.clone()),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
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

#[test]
fn middle_metrics_helpers_reject_unresolved_middle_policies() -> Result<(), String> {
    let (database, id) = fixture_database()?;
    let face = test_metadata();
    for kind in [PolicyKind::Middle, PolicyKind::AlignmentMiddle] {
        let policy = test_policy(kind);
        assert_eq!(super::metrics_shift(&database, id, &face, policy), 0.0);
        assert_eq!(
            super::css_baseline_shift(&database, id, &face, policy, 1.0),
            None
        );
        assert_eq!(
            super::legacy_glyph_shift(&database, id, policy, 8.0, -2.0, 4.0),
            0.0
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn baseline_targets_keep_hanging_and_middle_contracts() -> Result<(), String> {
    let (database, id) = fixture_database()?;
    let face = test_metadata();
    let hanging = test_policy(PolicyKind::Hanging);
    assert_eq!(
        super::css_baseline_target(&database, id, &face, hanging, 10.0),
        Some(6.4)
    );
    assert_eq!(
        super::legacy_glyph_shift(&database, id, hanging, 8.0, -2.0, 6.4),
        0.0
    );
    for kind in [PolicyKind::Middle, PolicyKind::AlignmentMiddle] {
        assert_eq!(
            super::css_baseline_target(&database, id, &face, test_policy(kind), 10.0),
            None
        );
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[test]
fn unavailable_layout_metrics_leave_baseline_unshifted() -> Result<(), String> {
    let (database, id) = fixture_database()?;
    let policy = test_policy(PolicyKind::Mathematical);
    assert_eq!(
        super::css_baseline_target(&database, id, &test_metadata(), policy, 10.0),
        None
    );
    assert_eq!(
        super::legacy_glyph_shift(&database, id, policy, 8.0, -2.0, 4.0),
        0.0
    );
    Ok(())
}

fn fixture_database() -> Result<(resvg::usvg::fontdb::Database, resvg::usvg::fontdb::ID), String> {
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_data(crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES.to_vec());
    let id = database.faces().next().ok_or("fixture font missing")?.id;
    Ok((database, id))
}

fn test_policy(kind: PolicyKind) -> Policy {
    Policy {
        kind,
        font_size: FIXTURE_FONT_SIZE,
        css_scale: Some(1.0),
        primary_shift: None,
    }
}

fn test_metadata() -> super::FaceMetadata {
    super::FaceMetadata {
        upem: FIXTURE_UNITS_PER_EM,
        unsupported: false,
        ascent: FIXTURE_ASCENT,
        descent: FIXTURE_DESCENT,
        x_height: None,
    }
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
