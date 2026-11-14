use std::collections::HashSet;

use crate::markdown::svg_rasterize::{
    GeneratedClipProvenance, font::html_font_db_for_text, html_glyph::HtmlGlyphRasterizer,
    provenance,
};
use resvg::usvg::{Node, Options, Tree};

const SAMPLE: &str = "Runtime";
const SOURCE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>
.ishikawa .ishikawa-label { font-size: 16px; dominant-baseline: middle; }
@keyframes edge-animation-frame { 0% { stroke-dashoffset: 0; } to { stroke-dashoffset: 1; } }
</style><g class="ishikawa"><text id="run" class="ishikawa-label" x="0" y="20" font-family="Trebuchet MS"><tspan x="0" dy="0">Runtime</tspan></text></g></svg>"#;

#[test]
fn nested_zero_dy_context_admits_dominant_middle_glyphs_only() -> Result<(), String> {
    let tree = tree(SOURCE)?;
    assert!(super::has_nested_middle_candidate(&tree));
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("nested source text missing".to_owned());
    };
    let source_context =
        provenance::authored_svg_context(SOURCE, &GeneratedClipProvenance::default(), false)
            .map_err(|error| error.to_string())?;
    let rasterizer = HtmlGlyphRasterizer::with_resvg_text_and_source_context(
        tree.fontdb(),
        HashSet::new(),
        &source_context.text,
    );
    let policy = super::resolved_policy(&rasterizer, text)?
        .ok_or("nested dominant-middle policy rejected despite source proof".to_owned())?;
    assert_eq!(policy.kind, super::PolicyKind::Middle);
    assert!(matches!(
        rasterizer.prepare(text)?,
        super::super::Prepared::Ready(_)
    ));
    assert_direct_text_remains_unadmitted()?;
    assert_explicit_alignment_middle_remains_unadmitted()?;
    Ok(())
}

fn assert_direct_text_remains_unadmitted() -> Result<(), String> {
    let source = SOURCE.replace("<tspan x=\"0\" dy=\"0\">Runtime</tspan>", "Runtime");
    let tree = tree(&source)?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("direct source text missing".to_owned());
    };
    assert!(super::has_nested_middle_candidate(&tree));
    assert!(super::uniform_policy_with_nested_context(text, false).is_none());
    assert!(super::resolved_policy(&HtmlGlyphRasterizer::new(tree.fontdb()), text)?.is_none());
    let context =
        provenance::authored_svg_context(&source, &GeneratedClipProvenance::default(), false)
            .map_err(|error| error.to_string())?;
    assert!(!context.text.matches(text));
    assert!(!context.text.matches_additional(text));
    let rasterizer = HtmlGlyphRasterizer::with_resvg_text_and_source_context(
        tree.fontdb(),
        HashSet::new(),
        &context.text,
    );
    assert!(super::resolved_policy(&rasterizer, text)?.is_none());
    Ok(())
}

#[test]
fn nested_middle_rejects_absent_and_mismatched_context() -> Result<(), String> {
    let tree = tree(SOURCE)?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("nested source text missing".to_owned());
    };
    assert!(super::uniform_policy_with_nested_context(text, false).is_none());
    assert!(super::resolved_policy(&HtmlGlyphRasterizer::new(tree.fontdb()), text)?.is_none());
    let other_source = SOURCE.replace(SAMPLE, "Different");
    let context =
        provenance::authored_svg_context(&other_source, &GeneratedClipProvenance::default(), false)
            .map_err(|error| error.to_string())?;
    let rasterizer = HtmlGlyphRasterizer::with_resvg_text_and_source_context(
        tree.fontdb(),
        HashSet::new(),
        &context.text,
    );
    assert!(!context.text.matches(text));
    assert!(!context.text.matches_additional(text));
    assert!(super::resolved_policy(&rasterizer, text)?.is_none());
    Ok(())
}

fn assert_explicit_alignment_middle_remains_unadmitted() -> Result<(), String> {
    let source = SOURCE.replace("dominant-baseline: middle;", "alignment-baseline: middle;");
    let tree = tree(&source)?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("explicit-alignment text missing".to_owned());
    };
    assert!(!super::has_nested_middle_candidate(&tree));
    assert!(super::uniform_policy_with_nested_context(text, true).is_none());
    Ok(())
}

fn tree(source: &str) -> Result<Tree, String> {
    Tree::from_str(
        source,
        &Options {
            fontdb: html_font_db_for_text("Trebuchet MS", SAMPLE),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}
