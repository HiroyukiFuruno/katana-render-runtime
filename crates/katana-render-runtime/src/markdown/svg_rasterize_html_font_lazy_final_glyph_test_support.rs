use crate::markdown::svg_rasterize::SvgRasterizeOps;

use super::MAX_FINAL_GLYPH_RECORDS;

type AuditRaster = Result<
    crate::markdown::svg_rasterize::RasterizedSvg,
    crate::markdown::svg_rasterize::SvgRasterizeError,
>;

pub(super) const PRIMARY_TEST_FONT_SIZE: usize = 16;
pub(super) const ALTERNATE_TEST_FONT_SIZE: usize = 17;
const FONT_AUDIT_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32"><text id="audit-run" x="1" y="24" font-size="16">AV</text></svg>"#;

pub(super) fn font_tree(
    database: resvg::usvg::fontdb::Database,
) -> Result<resvg::usvg::Tree, String> {
    parse_font_tree(FONT_AUDIT_SVG, database)
}

fn parse_font_tree(
    svg: &str,
    database: resvg::usvg::fontdb::Database,
) -> Result<resvg::usvg::Tree, String> {
    let family = database
        .faces()
        .find_map(|face| face.families.first().map(|family| family.0.clone()))
        .ok_or("fixture database has no font family")?;
    let options = resvg::usvg::Options {
        font_family: family,
        fontdb: std::sync::Arc::new(database),
        ..Default::default()
    };
    resvg::usvg::Tree::from_str(svg, &options).map_err(|error| error.to_string())
}

pub(super) fn text_span(
    tree: &resvg::usvg::Tree,
) -> Result<(&resvg::usvg::Text, &resvg::usvg::layout::Span), String> {
    let Some(resvg::usvg::Node::Text(text)) = tree.node_by_id("audit-run") else {
        return Err("font audit fixture text is missing".to_owned());
    };
    let span = text
        .layouted()
        .first()
        .ok_or("font audit fixture span is missing")?;
    Ok((text, span))
}

#[test]
fn glyph_audit_fixture_requires_a_font_family() {
    let database = resvg::usvg::fontdb::Database::new();
    assert_eq!(
        font_tree(database).err().as_deref(),
        Some("fixture database has no font family")
    );
}

#[test]
fn glyph_audit_fixture_rejects_malformed_svg_with_a_real_font() -> Result<(), String> {
    let (database, _) = super::variations_tests::bundled_face()?;
    let invalid = "<svg";
    let expected = resvg::usvg::Tree::from_str(invalid, &resvg::usvg::Options::default())
        .err()
        .ok_or("malformed SVG parsed")?;
    assert_eq!(
        parse_font_tree(invalid, database).err(),
        Some(expected.to_string())
    );
    assert!(audit_svg(invalid).is_err());
    Ok(())
}

#[test]
fn glyph_audit_fixture_rejects_a_shape_in_place_of_text() -> Result<(), String> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="24"><rect id="audit-run" width="16" height="12"/></svg>"#;
    let (database, _) = super::variations_tests::bundled_face()?;
    let tree = parse_font_tree(svg, database)?;
    assert!(matches!(
        tree.node_by_id("audit-run"),
        Some(resvg::usvg::Node::Path(_))
    ));
    assert_eq!(
        text_span(&tree).err().as_deref(),
        Some("font audit fixture text is missing")
    );
    Ok(())
}

#[test]
fn glyph_audit_root_lookup_rejects_missing_or_embedded_only_trees() {
    for audit in [
        serde_json::json!({}),
        serde_json::json!({"finalGlyphTrees": [{"treePath": "root/image[0]"}]}),
    ] {
        assert_eq!(
            root_tree(&audit).err().as_deref(),
            Some("root glyph tree evidence is absent")
        );
    }
}

#[test]
fn face_observation_reports_missing_glyph_size_for_another_real_face() -> Result<(), String> {
    let (mut database, first_face) = super::variations_tests::bundled_face()?;
    let second_face = *database
        .load_font_source(resvg::usvg::fontdb::Source::Binary(std::sync::Arc::new(
            crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES.to_vec(),
        )))
        .first()
        .ok_or("second bundled font face did not load")?;
    assert_ne!(first_face, second_face);
    let tree = font_tree(database)?;
    let (_, span) = text_span(&tree)?;
    let glyph = span
        .positioned_glyphs
        .first()
        .ok_or("fixture glyph missing")?;
    let other_face = [first_face, second_face]
        .into_iter()
        .find(|id| span.positioned_glyphs.iter().all(|glyph| glyph.font != *id))
        .ok_or("fixture span uses both faces")?;
    assert!(tree.fontdb().face(other_face).is_some());
    let result = super::variations::face_observation(tree.fontdb(), span, other_face, glyph);
    assert_eq!(
        result.err(),
        Some(format!("missing glyph font size for face {other_face:?}"))
    );
    Ok(())
}

pub(super) fn svg_with_many_text_spans() -> String {
    let spans = (0..=MAX_FINAL_GLYPH_RECORDS)
        .map(|index| {
            let size = if index % 2 == 0 {
                PRIMARY_TEST_FONT_SIZE
            } else {
                ALTERNATE_TEST_FONT_SIZE
            };
            format!(r#"<tspan font-size="{size}">A</tspan>"#)
        })
        .collect::<String>();
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><text x="0" y="20">{spans}</text></svg>"#
    )
}

pub(super) fn svg_with_repeated_glyph_text() -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="24"><text x="1" y="20"><tspan font-size="{PRIMARY_TEST_FONT_SIZE}">A</tspan><tspan font-size="{ALTERNATE_TEST_FONT_SIZE}">A</tspan></text></svg>"#
    )
}

pub(super) fn audit_svg(svg: &str) -> Result<serde_json::Value, String> {
    let (render, audit) = audit_render(&mut || SvgRasterizeOps::rasterize_html_svg(svg, 1.0))?;
    render.map_err(|error| error.to_string())?;
    Ok(audit)
}

fn audit_render(
    operation: &mut dyn FnMut() -> AuditRaster,
) -> Result<(AuditRaster, serde_json::Value), String> {
    SvgRasterizeOps::with_actual_font_source_audit(operation).map_err(|error| error.to_string())
}

#[cfg(unix)]
#[test]
fn glyph_audit_reports_non_utf8_source_metadata() -> Result<(), String> {
    use std::os::unix::ffi::OsStringExt;

    let path = std::path::PathBuf::from(std::ffi::OsString::from_vec(b"font-\xff.ttf".to_vec()));
    let mut database = resvg::usvg::fontdb::Database::new();
    let bytes = crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
    let source = resvg::usvg::fontdb::Source::SharedFile(path, std::sync::Arc::new(bytes));
    let ids = database.load_font_source(source);
    assert_eq!(ids.len(), 1);
    let result = audit_render(&mut || {
        super::super::record_actual_selected_face(&database, ids.first().copied());
        SvgRasterizeOps::rasterize_html_svg(FONT_AUDIT_SVG, 1.0)
    });
    assert_eq!(
        result.err().as_deref(),
        Some("path contains invalid UTF-8 characters")
    );
    Ok(())
}

pub(super) fn root_tree(audit: &serde_json::Value) -> Result<&serde_json::Value, String> {
    audit["finalGlyphTrees"]
        .as_array()
        .and_then(|trees| trees.iter().find(|tree| tree["treePath"] == "root"))
        .ok_or_else(|| "root glyph tree evidence is absent".to_owned())
}
