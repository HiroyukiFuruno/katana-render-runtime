use super::super::multi_chunk_alignment;
use super::{
    CHUNK_COUNT, HtmlGlyphRasterizer, Prepared, anchor, fixture_text, fixture_tree_with_content,
    font_fixture,
};

const CONTENT: &str =
    "<tspan x='75.02' y='40.503'>Waves</tspan><tspan x='95.02' y='60.503'>é</tspan>";
type GlyphIdentity = (u16, resvg::usvg::fontdb::ID, f32);

#[test]
fn single_scalar_row_keeps_both_chunk_origins_and_glyphs() -> Result<(), String> {
    let fixture = font_fixture("single-scalar")?;
    let tree = fixture_tree_with_content(&fixture, "", CONTENT)?;
    let text = fixture_text(&tree)?;
    let Prepared::Ready(mut spans) = HtmlGlyphRasterizer::new(tree.fontdb()).prepare(text)? else {
        return Err("single scalar fixture was not prepared".to_owned());
    };
    assert_eq!(text.chunks().len(), CHUNK_COUNT);
    assert_eq!(spans.len(), CHUNK_COUNT);
    assert_eq!(text.chunks()[1].text().chars().count(), 1);
    assert_eq!(spans[1].glyphs.len(), 1);
    assert!(spans.iter().all(|span| span.x_alignment.is_some()));
    let before = glyph_identity(&spans);
    for span in &mut spans {
        span.x_alignment = None;
    }
    assert!(multi_chunk_alignment::apply(text, &mut spans));
    assert!(spans.iter().all(|span| span.x_alignment.is_some()));
    assert_eq!(glyph_identity(&spans), before);
    Ok(())
}

fn glyph_identity(spans: &[super::PreparedSpan]) -> Vec<Vec<GlyphIdentity>> {
    spans
        .iter()
        .map(|span| {
            span.glyphs
                .iter()
                .zip(&span.glyph_ids)
                .map(|(glyph, id)| (*id, glyph.font, glyph.transform().ty))
                .collect()
        })
        .collect()
}

#[test]
fn empty_input_remains_ineligible() -> Result<(), String> {
    let fixture = font_fixture("empty")?;
    let tree = fixture_tree_with_content(&fixture, "", CONTENT)?;
    let text = fixture_text(&tree)?;
    assert!(!anchor::nonempty_ordered_scalar_per_glyph("", &[]));
    assert!(!multi_chunk_alignment::apply(text, &mut []));
    Ok(())
}
