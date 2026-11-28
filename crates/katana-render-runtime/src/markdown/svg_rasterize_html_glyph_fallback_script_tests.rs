use super::super::super::{HtmlGlyphRasterizer, Prepared, PreparedSpan};
use super::super::layout;
use super::{TITLE, assert_run_layout_metrics, text, tree};

#[cfg(target_os = "macos")]
#[test]
fn common_space_inherits_previous_script_across_fallback_and_splits_before_latin()
-> Result<(), String> {
    assert_title_space_shaping_boundary()
}

#[cfg(target_os = "macos")]
fn assert_title_space_shaping_boundary() -> Result<(), String> {
    let tree = tree(TITLE, "")?;
    let text = text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(prepared) = rasterizer.prepare(text)? else {
        return Err("mixed fallback title was not prepared".to_owned());
    };
    assert_title_space_boundary(text, &prepared)?;
    let layouts =
        layout(tree.fontdb(), text, &prepared)?.ok_or("fallback run layout was rejected")?;
    assert_run_layout_metrics(tree.fontdb(), text, &prepared, &layouts)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn assert_title_space_boundary(
    text: &resvg::usvg::Text,
    prepared: &[PreparedSpan],
) -> Result<(), String> {
    let source_glyphs = &text
        .layouted()
        .first()
        .ok_or("source layout is missing")?
        .positioned_glyphs;
    let scripts =
        super::super::resolved_scripts(source_glyphs).ok_or("source scripts are missing")?;
    let mut offset = 0;
    for span in prepared {
        if assert_span_space_boundary(source_glyphs, &scripts, span, offset)? {
            return Ok(());
        }
        offset += span.glyphs.len();
    }
    Err("space and API were not in one prepared font run".to_owned())
}

#[cfg(target_os = "macos")]
fn assert_span_space_boundary(
    source_glyphs: &[resvg::usvg::layout::PositionedGlyph],
    scripts: &[unicode_script::Script],
    span: &PreparedSpan,
    offset: usize,
) -> Result<bool, String> {
    let end = offset + span.glyphs.len();
    let source_run = source_glyphs
        .get(offset..end)
        .ok_or("prepared glyphs exceed source glyphs")?;
    let fragments =
        super::super::matching_script_fragments(source_run, span, &scripts[offset..end])
            .ok_or("source glyphs did not match prepared span")?;
    let run_text = span
        .glyphs
        .iter()
        .map(|glyph| glyph.text.as_str())
        .collect::<String>();
    let Some(space_index) = run_text.find(" API") else {
        return Ok(false);
    };
    assert_space_script_boundary(scripts, offset, &run_text, space_index);
    assert_space_latin_fragments(&fragments);
    Ok(true)
}

#[cfg(target_os = "macos")]
fn assert_space_script_boundary(
    scripts: &[unicode_script::Script],
    offset: usize,
    run_text: &str,
    space_index: usize,
) {
    assert_eq!(
        scripts[offset + run_text[..space_index].chars().count()],
        scripts[offset + run_text[..space_index].chars().count() - 1]
    );
    assert_ne!(
        scripts[offset + run_text[..space_index + 1].chars().count()],
        scripts[offset + run_text[..space_index].chars().count()]
    );
}

#[cfg(target_os = "macos")]
fn assert_space_latin_fragments(fragments: &[super::super::ScriptFragment]) {
    assert!(
        fragments
            .windows(2)
            .any(|pair| { pair[0].text == " " && pair[1].text.starts_with("API") })
    );
}
