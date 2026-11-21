use super::{
    additional_middle_layout_supported, layout_supported, layout_supported_with_zero_first_dy,
};
use crate::markdown::svg_rasterize::{
    GeneratedClipProvenance,
    provenance::{self, AuthoredTextContext},
};
use resvg::usvg::{Node, Options, Text, Tree};

const FONT_SIZE: f32 = 14.0;
const FIRST_BASELINE: f32 = 20.0;
const LINE_ADVANCE: f32 = 16.8;
const MULTILINE_CHUNK_COUNT: usize = 2;
const THREE_LINE_CHUNK_COUNT: usize = 3;
const FONT_FAMILY: &str = "Noto Sans";
const FLAT: &str = "AB";
const MULTILINE: &str = "<tspan x='0' dy='0'>AB</tspan><tspan x='0' dy='16.8'>CD</tspan>";
const THREE_LINES: &str = "<tspan x='0' dy='0'>AB</tspan><tspan x='0' dy='16.8'>CD</tspan><tspan x='0' dy='16.8'>EF</tspan>";
#[cfg(target_os = "macos")]
const FILE_FONT_FAMILY: &str = "Trebuchet MS";

#[path = "svg_rasterize_html_glyph_baseline_middle_proof_tests.rs"]
mod proof;
#[path = "svg_rasterize_html_glyph_baseline_middle_text_path_tests.rs"]
mod text_path_tests;

#[test]
fn flat_middle_keeps_the_original_zero_baseline_correction() -> Result<(), String> {
    for attributes in ["", "dy='0'"] {
        let tree = file_fixture(FLAT, attributes)?;
        let text = text(&tree)?;
        assert!(!layout_supported_with_zero_first_dy(text, false));
        assert!(!additional_middle_layout_supported(text));
        assert!(!layout_supported(text));
        let context = authored_context(FLAT, attributes, FILE_FONT_FAMILY)?;
        assert!(!context.matches(text));
        assert!(!context.matches_additional(text));
        let rasterizer = rasterizer(&tree, Some(&context));
        assert!(super::super::resolved_policy(&rasterizer, text)?.is_none());
        assert_prepared_line_origins(&tree, &context, 0.0, 1)?;
    }
    Ok(())
}

#[test]
fn explicit_chunk_line_breaks_admit_uniform_middle_text() -> Result<(), String> {
    let tree = fixture(MULTILINE, "")?;
    let text = text(&tree)?;
    assert_eq!(text.chunks().len(), MULTILINE_CHUNK_COUNT);
    assert!(text.dy().contains(&LINE_ADVANCE));
    assert!(!layout_supported_with_zero_first_dy(text, false));
    assert!(additional_middle_layout_supported(text));
    assert!(!layout_supported(text));
    assert_proven_admission(&tree, MULTILINE, "")?;
    Ok(())
}

#[test]
fn additional_lane_rejects_mixed_sizes_spacing_and_rotation() -> Result<(), String> {
    for attributes in [
        "letter-spacing='1'",
        "word-spacing='1'",
        "rotate='3'",
        "dx='1'",
        "alignment-baseline='middle'",
        "transform='rotate(10)'",
        "transform='scale(2 1)'",
        "text-decoration='underline'",
    ] {
        assert_additional_rejected(MULTILINE, attributes)?;
    }
    for body in [
        "<tspan x='0' dy='0' font-size='12'>AB</tspan><tspan x='0' dy='16.8'>CD</tspan>",
        "<tspan x='0' dy='0' font-family='serif'>AB</tspan><tspan x='0' dy='16.8'>CD</tspan>",
        "<tspan x='0' dy='0' dominant-baseline='central'>AB</tspan><tspan x='0' dy='16.8'>CD</tspan>",
        "<tspan x='0' dy='0' textLength='80'>AB</tspan><tspan x='0' dy='16.8'>CD</tspan>",
    ] {
        assert_additional_rejected(body, "")?;
    }
    Ok(())
}

#[test]
fn additional_lane_rejects_mid_chunk_dy_and_unreset_line_breaks() -> Result<(), String> {
    assert_additional_rejected(&MULTILINE.replace("dy='0'", "dy='0 1'"), "")?;
    assert_additional_rejected("<tspan>AB</tspan><tspan y='40'>CD</tspan>", "")?;
    Ok(())
}

#[test]
fn existing_nonzero_first_dy_lane_keeps_its_previous_admission() -> Result<(), String> {
    let tree = fixture(FLAT, "dy='1'")?;
    let text = text(&tree)?;
    assert!(layout_supported_with_zero_first_dy(text, false));
    assert!(layout_supported(text));
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn additional_lane_rejects_resolved_fallback_faces() -> Result<(), String> {
    for body in [
        "<tspan x='0' dy='0'>AB動物</tspan><tspan x='0' dy='16.8'>CD</tspan>",
        "<tspan x='0' dy='0'>AB</tspan><tspan x='0' dy='16.8'>CD動物</tspan>",
        "<tspan x='0' dy='0'>動物AB</tspan><tspan x='0' dy='16.8'>CD</tspan>",
    ] {
        let tree = file_fixture(body, "")?;
        let text = text(&tree)?;
        assert_eq!(text.chunks().len(), MULTILINE_CHUNK_COUNT);
        let faces = text
            .layouted()
            .iter()
            .flat_map(|span| &span.positioned_glyphs)
            .map(|glyph| glyph.font)
            .collect::<std::collections::HashSet<_>>();
        assert!(faces.len() > 1, "fixture must use a fallback face: {body}");
        assert!(!additional_middle_layout_supported(text), "{body}");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn explicit_multiline_reuses_the_existing_primary_baseline_shift() -> Result<(), String> {
    let old = file_fixture(FLAT, "dy='1'")?;
    let expected = primary_shift(&old, None)?;
    for (body, count) in [
        (MULTILINE, MULTILINE_CHUNK_COUNT),
        (THREE_LINES, THREE_LINE_CHUNK_COUNT),
    ] {
        let tree = file_fixture(body, "")?;
        let context = authored_context(body, "", FILE_FONT_FAMILY)?;
        assert!(context.matches_additional(text(&tree)?));
        assert_eq!(primary_shift(&tree, Some(&context))?, expected);
        assert_prepared_line_origins(&tree, &context, expected, count)?;
    }
    Ok(())
}

fn assert_additional_rejected(body: &str, attributes: &str) -> Result<(), String> {
    let tree = fixture(body, attributes)?;
    assert!(
        !additional_middle_layout_supported(text(&tree)?),
        "{body} {attributes}"
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn primary_shift(tree: &Tree, context: Option<&AuthoredTextContext>) -> Result<f32, String> {
    let rasterizer = rasterizer(tree, context);
    super::super::resolved_policy(&rasterizer, text(tree)?)?
        .and_then(|policy| policy.primary_shift)
        .ok_or_else(|| "middle primary correction missing".to_owned())
}

#[cfg(target_os = "macos")]
fn assert_prepared_line_origins(
    tree: &Tree,
    context: &AuthoredTextContext,
    expected: f32,
    count: usize,
) -> Result<(), String> {
    let rasterizer = rasterizer(tree, Some(context));
    let super::super::super::Prepared::Ready(spans) = rasterizer.prepare(text(tree)?)? else {
        return Err("middle line fixture was not prepared".to_owned());
    };
    assert_eq!(spans.len(), count);
    let mut origins = Vec::new();
    for span in spans {
        origins.push(prepared_origin(&span, expected)?);
    }
    assert_eq!(origins.len(), count);
    assert_line_gaps(&origins);
    Ok(())
}

#[cfg(target_os = "macos")]
fn prepared_origin(
    span: &super::super::super::PreparedSpan,
    expected: f32,
) -> Result<(f32, f32), String> {
    assert_eq!(span.baseline_shift, expected);
    let glyph = span.glyphs.first().ok_or("middle line glyph missing")?;
    let raw_y = glyph.transform().ty;
    let painted = super::super::super::paint::normalized_transform(span, glyph, 1.0);
    assert_eq!(painted.ty, raw_y + expected);
    Ok((raw_y, painted.ty))
}

#[cfg(target_os = "macos")]
fn assert_line_gaps(origins: &[(f32, f32)]) {
    for pair in origins.windows(MULTILINE_CHUNK_COUNT) {
        let first = pair[0];
        let second = pair[1];
        let raw_gap = second.0 - first.0;
        let painted_gap = second.1 - first.1;
        let rounding_bound = first.1.abs().max(second.1.abs()) * f32::EPSILON;
        assert!((raw_gap - LINE_ADVANCE).abs() <= rounding_bound);
        assert!((painted_gap - raw_gap).abs() <= rounding_bound);
    }
}

#[cfg(target_os = "macos")]
fn rasterizer<'a>(
    tree: &'a Tree,
    context: Option<&'a AuthoredTextContext>,
) -> super::super::super::HtmlGlyphRasterizer<'a> {
    match context {
        Some(context) => {
            super::super::super::HtmlGlyphRasterizer::with_resvg_text_and_source_context(
                tree.fontdb(),
                std::collections::HashSet::new(),
                context,
            )
        }
        None => super::super::super::HtmlGlyphRasterizer::new(tree.fontdb()),
    }
}

fn assert_proven_admission(tree: &Tree, body: &str, attributes: &str) -> Result<(), String> {
    let text = text(tree)?;
    assert!(super::super::has_nested_middle_candidate(tree));
    assert!(super::super::uniform_policy(text).is_none());
    let context = authored_context(body, attributes, FONT_FAMILY)?;
    assert!(!context.matches(text));
    assert!(context.matches_additional(text));
    let admitted = super::super::policy::uniform_policy_with_context(
        text,
        false,
        context.matches_additional(text),
    );
    assert!(admitted.is_some());
    Ok(())
}

fn authored_context(
    body: &str,
    attributes: &str,
    family: &str,
) -> Result<AuthoredTextContext, String> {
    provenance::authored_svg_context(
        &source(body, attributes, family),
        &GeneratedClipProvenance::default(),
        false,
    )
    .map(|context| context.text)
    .map_err(|error| error.to_string())
}

fn text(tree: &Tree) -> Result<&Text, String> {
    match tree.node_by_id("run") {
        Some(Node::Text(text)) => Ok(text),
        _ => Err("middle lane text missing".to_owned()),
    }
}

fn fixture(body: &str, attributes: &str) -> Result<Tree, String> {
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_data(crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES.to_vec());
    parse(body, attributes, FONT_FAMILY, std::sync::Arc::new(database))
}

#[cfg(target_os = "macos")]
fn file_fixture(body: &str, attributes: &str) -> Result<Tree, String> {
    let database = crate::markdown::svg_rasterize::font::html_font_db_for_text(
        "Trebuchet MS, Hiragino Kaku Gothic ProN",
        "ABCDEF動物",
    );
    parse(body, attributes, FILE_FONT_FAMILY, database)
}

fn parse(
    body: &str,
    attributes: &str,
    family: &str,
    database: std::sync::Arc<resvg::usvg::fontdb::Database>,
) -> Result<Tree, String> {
    Tree::from_str(
        &source(body, attributes, family),
        &Options {
            fontdb: database,
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}

fn source(body: &str, attributes: &str, family: &str) -> String {
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg'><text id='run' x='0' y='{FIRST_BASELINE}' \
         font-family='{family}' font-size='{FONT_SIZE}' dominant-baseline='middle' \
         {attributes}>{body}</text></svg>"
    )
}
