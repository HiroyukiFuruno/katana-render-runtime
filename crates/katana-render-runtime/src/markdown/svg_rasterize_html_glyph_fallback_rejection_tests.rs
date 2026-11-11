use super::super::super::{HtmlGlyphRasterizer, Prepared, PreparedSpan};
use super::super::{layout, metrics, scripts::ScriptFragment};
use super::{FONT_FAMILIES, TITLE, text, tree_with_fonts, tree_with_physical_noto};
use crate::markdown::svg_rasterize::text_shaping::FontShape;
use resvg::usvg::Tree;
use std::collections::HashSet;

const REJECTION_CASE_COUNT: usize = 4;
const NORMAL_FONT_WEIGHT: u16 = 400;

#[cfg(target_os = "macos")]
#[test]
fn fallback_run_layout_rejects_word_spacing_ligatures_rtl_and_nonuniform_scale()
-> Result<(), String> {
    for case in rejection_cases() {
        assert_rejection_case(case)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn fallback_alignment_rejects_distinct_authored_baselines() -> Result<(), String> {
    const CONTENT: &str = "A日<tspan alignment-baseline='hanging'>B</tspan>";
    let tree = tree_with_physical_noto(CONTENT, "", "Noto Sans", "Noto Sans")?;
    let text = text(&tree)?;
    let prepared = prepare_text(&tree, text, CONTENT)?;
    assert_prepared_fallback(&prepared, CONTENT);
    let mut glyphs = text
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs);
    let first = glyphs.next().ok_or("baseline fixture has no glyphs")?;
    assert!(glyphs.any(|glyph| glyph.transform().ty != first.transform().ty));
    assert!(layout(tree.fontdb(), text, &prepared)?.is_none());
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn fallback_combining_cluster_preserves_shaped_glyphs_without_run_alignment() -> Result<(), String>
{
    const CONTENT: &str = "A\u{301}日本";
    let tree = tree_with_physical_noto(CONTENT, "", "Noto Sans", "Noto Sans")?;
    let text = text(&tree)?;
    let prepared = prepare_text(&tree, text, CONTENT)?;
    assert_prepared_fallback(&prepared, CONTENT);
    assert_ligature_fixture(text, CONTENT);
    assert!(layout(tree.fontdb(), text, &prepared)?.is_none());
    assert!(
        prepared
            .iter()
            .all(|span| span.fallback_alignment.is_none())
    );
    let original = text
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
        .map(|glyph| (glyph.font, glyph.id, glyph.transform()))
        .collect::<Vec<_>>();
    let actual = prepared
        .iter()
        .flat_map(|span| &span.glyphs)
        .map(|glyph| (glyph.font, glyph.id, glyph.transform()))
        .collect::<Vec<_>>();
    assert_eq!(actual, original);
    Ok(())
}

#[cfg(target_os = "macos")]
struct RejectionCase {
    content: &'static str,
    attributes: &'static str,
    source_family: &'static str,
    database_families: &'static str,
}

#[cfg(target_os = "macos")]
fn rejection_cases() -> [RejectionCase; REJECTION_CASE_COUNT] {
    [
        RejectionCase {
            content: TITLE,
            attributes: "word-spacing='1'",
            source_family: "Trebuchet MS",
            database_families: FONT_FAMILIES,
        },
        RejectionCase {
            content: "office日本",
            attributes: "",
            source_family: "Noto Sans",
            database_families: "Noto Sans",
        },
        RejectionCase {
            content: "abc אבג 日本",
            attributes: "",
            source_family: "Noto Sans",
            database_families: "Noto Sans",
        },
        RejectionCase {
            content: TITLE,
            attributes: "transform='scale(0.9999806707719402 1)'",
            source_family: "Trebuchet MS",
            database_families: FONT_FAMILIES,
        },
    ]
}

#[cfg(target_os = "macos")]
fn assert_rejection_case(case: RejectionCase) -> Result<(), String> {
    let tree = rejection_tree(&case)?;
    let text = text(&tree)?;
    let prepared = prepare_text(&tree, text, case.content)?;
    assert_prepared_fallback(&prepared, case.content);
    assert_case_preconditions(&tree, text, &prepared, &case)?;
    assert!(
        layout(tree.fontdb(), text, &prepared)?.is_none(),
        "unsupported fallback layout was admitted: {} {}",
        case.content,
        case.attributes
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn rejection_tree(case: &RejectionCase) -> Result<Tree, String> {
    if case.source_family == "Noto Sans" {
        tree_with_physical_noto(
            case.content,
            case.attributes,
            case.source_family,
            case.database_families,
        )
    } else {
        tree_with_fonts(
            case.content,
            case.attributes,
            case.source_family,
            case.database_families,
        )
    }
}

#[cfg(target_os = "macos")]
fn prepare_text(
    tree: &Tree,
    text: &resvg::usvg::Text,
    content: &str,
) -> Result<Vec<PreparedSpan>, String> {
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(prepared) = rasterizer.prepare(text)? else {
        return Err(format!("rejection fixture was not prepared: {content}"));
    };
    Ok(prepared)
}

#[cfg(target_os = "macos")]
fn assert_prepared_fallback(prepared: &[PreparedSpan], content: &str) {
    let fonts = prepared
        .iter()
        .map(|span| span.font)
        .collect::<HashSet<_>>();
    assert!(
        fonts.len() > 1,
        "rejection fixture did not use fallback faces: {content}"
    );
}

#[cfg(target_os = "macos")]
fn assert_case_preconditions(
    tree: &Tree,
    text: &resvg::usvg::Text,
    prepared: &[PreparedSpan],
    case: &RejectionCase,
) -> Result<(), String> {
    if case.content == "office日本" {
        assert_ligature_fixture(text, case.content);
    }
    if case.content.contains("אבג") {
        assert_rtl_fixture(tree, prepared)?;
    }
    assert_attribute_preconditions(text, case)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn assert_ligature_fixture(text: &resvg::usvg::Text, content: &str) {
    let source_glyphs = text
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
        .count();
    assert!(
        source_glyphs < content.chars().count(),
        "ligature fixture did not reduce source glyph count"
    );
    assert!(
        super::super::super::anchor::first_ordered_scalar_glyph(
            content,
            &text.layouted()[0].positioned_glyphs,
        )
        .is_none(),
        "ligature fixture did not form a multi-scalar glyph"
    );
}

#[cfg(target_os = "macos")]
fn assert_rtl_fixture(tree: &Tree, prepared: &[PreparedSpan]) -> Result<(), String> {
    assert!(
        prepared.iter().any(|span| rtl_metrics_rejected(tree, span)),
        "RTL fixture did not expose a run rejected by the LTR metrics helper"
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn rtl_metrics_rejected(tree: &Tree, span: &PreparedSpan) -> bool {
    let run_text = span
        .glyphs
        .iter()
        .map(|glyph| glyph.text.as_str())
        .collect::<String>();
    if !run_text
        .chars()
        .any(|character| matches!(character, '\u{0590}'..='\u{05ff}'))
    {
        return false;
    }
    let fragment = ScriptFragment {
        glyph_range: 0..span.glyphs.len(),
        text: run_text,
    };
    metrics::measure_fragment(
        tree.fontdb(),
        span,
        &fragment,
        &span.glyphs,
        FontShape::new(span.font_size, NORMAL_FONT_WEIGHT, false),
    )
    .is_none()
}

#[cfg(target_os = "macos")]
fn assert_attribute_preconditions(
    text: &resvg::usvg::Text,
    case: &RejectionCase,
) -> Result<(), String> {
    if case.attributes.contains("word-spacing=") {
        assert!(!super::super::super::alignment_source_span_supported(
            text.chunks()[0]
                .spans()
                .first()
                .ok_or("source span is missing")?
        ));
    }
    if case.attributes.contains("scale(") && case.attributes.contains(" 1)") {
        assert!(super::super::super::anchor::uniform_css_scale(text.abs_transform()).is_none());
    }
    Ok(())
}
