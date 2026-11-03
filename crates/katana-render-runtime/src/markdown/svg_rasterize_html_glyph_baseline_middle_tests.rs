#[cfg(target_os = "macos")]
use super::Policy;
use super::{FaceMetadata, raw_x_height, valid_face_metrics};
use crate::markdown::svg_rasterize::{
    font::html_font_db_for_text, html_glyph::HtmlGlyphRasterizer,
};
use resvg::usvg::{Node, Options, Tree};

const FONT_UNITS_PER_EM: f32 = 1000.0;
const FONT_ASCENT: f32 = 800.0;
const FONT_DESCENT: f32 = -200.0;
#[cfg(target_os = "macos")]
/* WHY: R2506 blob0のsource font-sizeをSkia paint sizeと分けて保持する。 */
const ARCHITECTURE_SOURCE_FONT_SIZE: f32 = 16.0;
#[cfg(target_os = "macos")]
/* WHY: 共有fixtureのSVG/cache source spanは従来のSkia paint sizeを保つ。 */
const ARCHITECTURE_CACHE_SPAN_FONT_SIZE: f32 = 15.99;
#[cfg(target_os = "macos")]
/* WHY: R2506 blob0のsource transform scaleを独立rawと同じ精度で固定する。 */
const ARCHITECTURE_CSS_SCALE: f32 = 0.999_940_34;
/* WHY: R2506 blob0の親text/tspan位置指定を同じSVG座標単位で再現する。 */
#[cfg(target_os = "macos")]
const ARCHITECTURE_TEXT_Y: f32 = -10.1;
#[cfg(target_os = "macos")]
const ARCHITECTURE_TSPAN_Y_EM: f32 = -0.1;
#[cfg(target_os = "macos")]
const ARCHITECTURE_TSPAN_DY_EM: f32 = 1.1;
#[cfg(target_os = "macos")]
/* WHY: 独立Chrome originはprepared paint後のCSS LayoutUnit化結果として照合する。 */
const ARCHITECTURE_CHROME_PAINT_ORIGIN: f32 = 19.984_375;
#[cfg(target_os = "macos")]
const ORIGIN_TOLERANCE: f32 = 0.0001;

#[test]
fn quantized_x_height_rejects_unusable_font_metrics() {
    for value in [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        -1.0,
        0.0,
        0.1,
        f32::from(u16::MAX) + 1.0,
    ] {
        assert_eq!(super::quantized_x_height(value), None);
    }
    assert_eq!(super::quantized_x_height(1.0), Some(1.0));
    assert_eq!(
        super::quantized_x_height(f32::from(u16::MAX)),
        Some(f32::from(u16::MAX))
    );
}

#[test]
fn raw_x_height_uses_rounded_font_metric_or_usvg_fallback() {
    for (metric, expected) in [
        (Some(7.49), Some(7.0)),
        (Some(7.5), Some(8.0)),
        (None, Some(450.0)),
        (Some(f32::NAN), Some(450.0)),
        (Some(f32::INFINITY), Some(450.0)),
    ] {
        assert_eq!(raw_x_height(&metadata(metric)), expected);
    }
    let mut invalid = metadata(None);
    invalid.descent = invalid.ascent;
    assert_eq!(raw_x_height(&invalid), None);
}

#[test]
fn middle_rejects_invalid_face_metrics() {
    assert!(valid_face_metrics(&metadata(Some(450.0))));
    let mut invalid = metadata(Some(450.0));
    invalid.upem = 0.0;
    assert!(!valid_face_metrics(&invalid));
    invalid = metadata(Some(450.0));
    invalid.ascent = f32::NAN;
    assert!(!valid_face_metrics(&invalid));
    invalid = metadata(Some(450.0));
    invalid.descent = f32::INFINITY;
    assert!(!valid_face_metrics(&invalid));
}

#[test]
fn middle_policy_rejects_a_binary_only_face() -> Result<(), String> {
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_data(crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES.to_vec());
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" x="0" y="20" dy="1" font-family="Noto Sans" dominant-baseline="middle">AB</text></svg>"#;
    let tree = Tree::from_str(
        source,
        &Options {
            fontdb: std::sync::Arc::new(database),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())?;
    assert_rejected_middle_policy(
        &tree,
        "binary middle text missing",
        MiddlePolicyCase::BinaryOnlyFace,
    )
}

enum MiddlePolicyCase {
    BinaryOnlyFace,
    MixedRequestedFonts,
}

fn assert_rejected_middle_policy(
    tree: &Tree,
    missing_message: &str,
    case: MiddlePolicyCase,
) -> Result<(), String> {
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err(missing_message.to_owned());
    };
    if matches!(case, MiddlePolicyCase::MixedRequestedFonts) {
        assert!(text.chunks()[0].spans().len() > 1);
    }
    assert!(super::super::uniform_policy(text).is_some());
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    assert!(super::super::resolved_policy(&rasterizer, text)?.is_none());
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn middle_policy_rejects_mixed_requested_font_descriptors() -> Result<(), String> {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" x="0" y="20" dy="1" dominant-baseline="middle"><tspan font-family="Trebuchet MS">AB</tspan><tspan font-family="Hiragino Kaku Gothic ProN">動物</tspan></text></svg>"#;
    let tree = middle_fixture_tree(source, "Trebuchet MS, Hiragino Kaku Gothic ProN", "AB動物")?;
    assert_rejected_middle_policy(
        &tree,
        "mixed middle text missing",
        MiddlePolicyCase::MixedRequestedFonts,
    )
}

#[cfg(target_os = "macos")]
#[test]
fn middle_policy_shares_primary_shift_with_fallback_glyph_runs() -> Result<(), String> {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" x="0" y="20" dy="1" font-family="Trebuchet MS" font-size="15.99" dominant-baseline="middle">AB動物</text></svg>"#;
    let tree = middle_fixture_tree(source, "Trebuchet MS, Hiragino Kaku Gothic ProN", "AB動物")?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("fallback middle text missing".to_owned());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let super::super::super::Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("fallback middle text was not prepared".to_owned());
    };
    let font_ids = spans
        .iter()
        .map(|span| span.font)
        .collect::<std::collections::HashSet<_>>();
    assert!(font_ids.len() > 1);
    let first_shift = spans.first().ok_or("middle spans missing")?.baseline_shift;
    assert!(spans.iter().all(|span| span.baseline_shift == first_shift));
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn middle_admits_same_style_svg_cache_source_spans() -> Result<(), String> {
    let tree = architecture_cache_tree()?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("middle cache text missing".to_owned());
    };
    assert_eq!(text.chunks().len(), 1);
    assert_eq!(text.chunks()[0].spans().len(), 2);
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    assert!(super::super::resolved_policy(&rasterizer, text)?.is_some());
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn resolved_middle_shift_matches_architecture_control_origin_delta() -> Result<(), String> {
    let (policy, span) = prepared_architecture_origin()?;
    assert_eq!(policy.css_scale, Some(ARCHITECTURE_CSS_SCALE));
    assert_eq!(policy.font_size, ARCHITECTURE_SOURCE_FONT_SIZE);
    let alignment = span.x_alignment.ok_or("middle origin alignment missing")?;
    assert_eq!(
        alignment.origin_snap_css_scale,
        Some(ARCHITECTURE_CSS_SCALE)
    );
    let glyph = span.glyphs.first().ok_or("middle origin glyph missing")?;
    let usvg_origin = glyph.transform().ty;
    assert!(
        usvg_origin.is_finite(),
        "usvg origin {usvg_origin} is non-finite"
    );
    let paint_origin =
        super::super::super::paint::normalized_transform(&span, glyph, ARCHITECTURE_CSS_SCALE).ty;
    let snapped_origin = super::super::super::anchor::trunc_layout_unit(paint_origin)
        .ok_or("middle origin could not be layout-unit snapped")?;
    assert!(
        (snapped_origin - ARCHITECTURE_CHROME_PAINT_ORIGIN).abs() < ORIGIN_TOLERANCE,
        "paint origin {paint_origin}, snapped origin {snapped_origin}, expected {ARCHITECTURE_CHROME_PAINT_ORIGIN}"
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn prepared_architecture_origin() -> Result<(Policy, super::super::super::PreparedSpan), String> {
    let tree = architecture_origin_tree()?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("middle origin text missing".to_owned());
    };
    if text.chunks().len() != 1 || text.chunks()[0].spans().len() != 1 {
        return Err("middle origin must use one source span".to_owned());
    }
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let policy = super::super::resolved_policy(&rasterizer, text)?
        .ok_or("middle origin policy rejected".to_owned())?;
    let super::super::super::Prepared::Ready(mut spans) = rasterizer.prepare(text)? else {
        return Err("middle origin text was not prepared".to_owned());
    };
    let span = spans.pop().ok_or("middle origin span missing")?;
    if !spans.is_empty() {
        return Err("middle origin prepared multiple spans".to_owned());
    }
    let primary_shift = policy
        .primary_shift
        .ok_or("middle origin policy omitted primary correction")?;
    assert_eq!(span.baseline_shift, primary_shift);
    Ok((policy, span))
}

#[cfg(target_os = "macos")]
fn middle_fixture_tree(source: &str, font_families: &str, sample: &str) -> Result<Tree, String> {
    let database = html_font_db_for_text(font_families, sample);
    Tree::from_str(
        source,
        &Options {
            fontdb: database,
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn architecture_cache_tree() -> Result<Tree, String> {
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><text id="run" x="0" y="20" dy="1" dominant-baseline="middle"><tspan font-family="Trebuchet MS" font-size="{ARCHITECTURE_CACHE_SPAN_FONT_SIZE}">SVG</tspan><tspan font-family="Trebuchet MS" font-size="{ARCHITECTURE_CACHE_SPAN_FONT_SIZE}"> cache</tspan></text></svg>"#
    );
    middle_fixture_tree(&source, "Trebuchet MS", "SVG cache")
}

#[cfg(target_os = "macos")]
fn architecture_origin_tree() -> Result<Tree, String> {
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><text id="run" y="{ARCHITECTURE_TEXT_Y}" transform="scale({ARCHITECTURE_CSS_SCALE})" dominant-baseline="middle"><tspan x="0" y="{ARCHITECTURE_TSPAN_Y_EM}em" dy="{ARCHITECTURE_TSPAN_DY_EM}em" font-family="Trebuchet MS" font-size="{ARCHITECTURE_SOURCE_FONT_SIZE}">Markdown</tspan></text></svg>"#
    );
    middle_fixture_tree(&source, "Trebuchet MS", "Markdown")
}

fn metadata(x_height: Option<f32>) -> FaceMetadata {
    FaceMetadata {
        upem: FONT_UNITS_PER_EM,
        unsupported: false,
        ascent: FONT_ASCENT,
        descent: FONT_DESCENT,
        x_height,
    }
}
