use super::row_diagnostic;
use crate::markdown::svg_rasterize::font_file_fixture;
use crate::markdown::svg_rasterize::html_glyph::{
    HtmlGlyphRasterizer, PreparedSpan,
    multi_chunk_alignment_tests::{fixture_text, font_fixture, nested_rows::nested_rows_tree},
};
use resvg::usvg::Tree;
use serde_json::{Value, json};

#[test]
fn nested_rows_report_source_layout_and_prepared_ranges() -> Result<(), String> {
    let (_font, tree, prepared) = prepared_fixture("valid", "")?;
    let text = fixture_text(&tree)?;
    let diagnostic = row_diagnostic(text, &prepared);
    assert_eq!(diagnostic["chunkCount"], json!(2));
    assert_eq!(diagnostic["layoutCount"], json!(prepared.len()));
    assert_eq!(diagnostic["preparedCount"], json!(prepared.len()));
    assert_eq!(diagnostic["rowsSupported"], json!(true));
    assert_eq!(diagnostic["rowInputs"], json!(true));
    let rows = diagnostic["rows"]
        .as_array()
        .ok_or("diagnostic rows missing")?;
    assert_eq!(rows.len(), text.chunks().len());
    let first_end = text.chunks()[0].spans().len();
    assert_eq!(first_end, 2);
    assert_row_detail(&rows[0], text.chunks()[0].text(), 0, first_end)?;
    assert_row_detail(&rows[1], text.chunks()[1].text(), first_end, prepared.len())?;
    Ok(())
}

fn assert_row_detail(
    row: &Value,
    expected_text: &str,
    span_start: usize,
    span_end: usize,
) -> Result<(), String> {
    assert_eq!(row["spanRange"], json!([span_start, span_end]));
    assert_eq!(row["rowInput"], json!(true));
    assert_eq!(row["detail"]["text"], json!(expected_text));
    for key in [
        "chunkSupported",
        "spansSupported",
        "chunkInput",
        "dySupported",
    ] {
        assert_eq!(row["detail"][key], json!(true), "{key}");
    }
    assert!(
        row["detail"]["layoutsMatch"]
            .as_array()
            .is_some_and(|matches| !matches.is_empty() && matches.iter().all(|item| item == true))
    );
    assert!(
        row["detail"]["spans"]
            .as_array()
            .is_some_and(|spans| spans.len() == span_end - span_start)
    );
    Ok(())
}

#[test]
fn diagnostic_exposes_guard_rejection_and_incomplete_preparation() -> Result<(), String> {
    let (_font, tree, prepared) = prepared_fixture("spaced", "letter-spacing='1'")?;
    let text = fixture_text(&tree)?;
    let rejected = row_diagnostic(text, &prepared);
    assert_eq!(rejected["rowsSupported"], json!(true));
    assert_eq!(rejected["rowInputs"], json!(false));
    for row in rejected["rows"].as_array().ok_or("rejected rows missing")? {
        assert_eq!(row["rowInput"], json!(false));
        assert_eq!(row["detail"]["chunkInput"], json!(false));
        assert_eq!(row["detail"]["spansSupported"], json!(false));
    }
    let incomplete = row_diagnostic(text, &prepared[..1]);
    assert_eq!(incomplete["rowsSupported"], json!(false));
    let rows = incomplete["rows"]
        .as_array()
        .ok_or("incomplete rows missing")?;
    assert_eq!(rows[1]["detail"]["spans"], Value::Null);
    assert_eq!(rows[1]["detail"]["layoutsMatch"], Value::Null);
    Ok(())
}

fn prepared_fixture(
    name: &str,
    attributes: &str,
) -> Result<(font_file_fixture::FontFileFixture, Tree, Vec<PreparedSpan>), String> {
    let font = font_fixture(&format!("row-diagnostic-{name}"))?;
    let tree = nested_rows_tree(&font, attributes, "")?;
    let text = fixture_text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let crate::markdown::svg_rasterize::html_glyph::Prepared::Ready(prepared) =
        rasterizer.prepare(text)?
    else {
        return Err("nested-row diagnostic fixture was not prepared".into());
    };
    Ok((font, tree, prepared))
}
