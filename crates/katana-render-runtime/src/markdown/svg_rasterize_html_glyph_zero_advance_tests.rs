use super::super::super::eligibility::eligible_source;
use super::super::{html_font_db_for_text, noto_face_ids, tree_with_database};
use super::{FontShape, NORMAL_FONT_WEIGHT, PreparedSpan, ScriptFragment, Tree, layout, metrics};
use super::{assert_prepared_fallback, prepare_text, text};
use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
use rstest::rstest;
use std::{path::Path, sync::Arc};
use write_fonts::{
    FontBuilder,
    read::{FontRef, TableProvider},
    types::Tag,
};

const FAMILY: &str = "Noto Sans";

#[rstest]
#[case::leading("A日本", false)]
#[case::trailing("日本A", true)]
fn fallback_run_with_a_real_zero_advance_glyph_keeps_original_positions(
    #[case] content: &str,
    #[case] zero_reaches_positioning: bool,
) -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    for (name, bytes, expected) in [
        ("normal.ttf", FONT_BYTES.to_vec(), true),
        ("zero.ttf", with_zero_advance()?, false),
    ] {
        let tree = fallback_tree(&directory.path().join(name), &bytes, content)?;
        let source = text(&tree)?;
        let spans = prepare_text(&tree, source, content)?;
        assert_prepared_fallback(&spans, content);
        let advance = latin_advance(&tree, &spans)?;
        assert_eq!(advance > 0.0, expected, "{name}: {advance}");
        let admitted = eligible_source(source, &spans).is_some();
        assert_eq!(admitted, expected || zero_reaches_positioning);
        assert_eq!(
            layout(tree.fontdb(), source, &spans)?.is_some(),
            expected,
            "{name}"
        );
        if !expected {
            assert_eq!(advance, 0.0);
            assert!(spans.iter().all(|span| span.fallback_alignment.is_none()));
        }
    }
    Ok(())
}

fn fallback_tree(path: &Path, bytes: &[u8], content: &str) -> Result<Tree, String> {
    std::fs::write(path, bytes).map_err(|error| error.to_string())?;
    let mut database = html_font_db_for_text(FAMILY, content);
    let existing = noto_face_ids(&database);
    let writable = Arc::make_mut(&mut database);
    for face in existing {
        writable.remove_face(face);
    }
    writable
        .load_font_file(path)
        .map_err(|error| error.to_string())?;
    tree_with_database(content, "", FAMILY, database)
}

fn latin_advance(tree: &Tree, spans: &[PreparedSpan]) -> Result<f32, String> {
    let span = spans
        .iter()
        .find(|span| span.glyphs.len() == 1 && span.glyphs[0].text == "A")
        .ok_or("isolated Latin glyph missing")?;
    let fragment = ScriptFragment {
        glyph_range: 0..1,
        text: "A".to_owned(),
    };
    let font = FontShape::new(span.font_size, NORMAL_FONT_WEIGHT, false);
    let advance = metrics::measure_fragment(tree.fontdb(), span, &fragment, &span.glyphs, font)
        .ok_or("Latin metrics unavailable")?;
    assert_eq!(advance.len(), 1);
    Ok(advance[0].0)
}

fn with_zero_advance() -> Result<Vec<u8>, String> {
    const HMTX: Tag = Tag::new(b"hmtx");
    const ADVANCE_BYTES: usize = 2;
    const LONG_METRIC_BYTES: usize = 4;
    let original = FontRef::new(FONT_BYTES).map_err(|error| error.to_string())?;
    let glyph = original
        .cmap()
        .map_err(|error| error.to_string())?
        .map_codepoint('A')
        .ok_or("Latin font mapping missing")?
        .to_u32();
    let long_metrics = original
        .hhea()
        .map_err(|error| error.to_string())?
        .number_of_h_metrics();
    assert!(glyph < u32::from(long_metrics));
    let offset = glyph as usize * LONG_METRIC_BYTES;
    let mut metrics = original
        .data_for_tag(HMTX)
        .ok_or("advance table missing")?
        .as_bytes()
        .to_vec();
    metrics[offset..offset + ADVANCE_BYTES].fill(0);
    let mut builder = FontBuilder::new();
    builder.add_raw(HMTX, &metrics);
    Ok(builder.copy_missing_tables(original).build())
}
