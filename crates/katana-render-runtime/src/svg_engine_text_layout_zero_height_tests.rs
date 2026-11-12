use super::super::{FONT, TestResult};
use super::{fixture_database, usvg};
use read_fonts::{FontRef, types::Tag};
use skrifa::MetadataProvider;

const HHEA_ASCENT_AND_DESCENT: std::ops::Range<usize> = 4..8;
const OS2_TYPO_ASCENT_AND_DESCENT: std::ops::Range<usize> = 68..72;
const OS2_WIN_ASCENT_AND_DESCENT: std::ops::Range<usize> = 74..78;
const SVG: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='80'><rect id='anchor' width='4' height='4'/><text id='sample' x='10' y='40' font-family='Noto Sans' font-size='20'>A</text></svg>";

#[test]
fn zero_font_line_height_does_not_drop_unrelated_svg_geometry() -> TestResult<()> {
    let original = parse_with_font(FONT)?;
    assert!(original.node_by_id("sample").is_some());
    let malformed = font_with_zero_line_height()?;
    assert_zero_line_height(&malformed)?;
    let candidate = parse_with_font(&malformed)?;
    assert!(candidate.node_by_id("anchor").is_some());
    assert!(candidate.node_by_id("sample").is_none());
    Ok(())
}

fn parse_with_font(bytes: &[u8]) -> TestResult<usvg::Tree> {
    let (fontdb, _) = fixture_database(bytes)?;
    let options = usvg::Options {
        fontdb,
        ..usvg::Options::default()
    };
    Ok(usvg::Tree::from_str(SVG, &options)?)
}

fn font_with_zero_line_height() -> TestResult<Vec<u8>> {
    let font = FontRef::new(FONT)?;
    let hhea = table_offset(&font, Tag::new(b"hhea"))?;
    let os2 = table_offset(&font, Tag::new(b"OS/2"))?;
    let mut bytes = FONT.to_vec();
    for (table, range) in [
        (hhea, HHEA_ASCENT_AND_DESCENT),
        (os2, OS2_TYPO_ASCENT_AND_DESCENT),
        (os2, OS2_WIN_ASCENT_AND_DESCENT),
    ] {
        bytes
            .get_mut(table + range.start..table + range.end)
            .ok_or("font line metric range is missing")?
            .fill(0);
    }
    Ok(bytes)
}

fn table_offset(font: &FontRef<'_>, tag: Tag) -> TestResult<usize> {
    font.table_directory()
        .table_records()
        .iter()
        .find(|record| record.tag() == tag)
        .map(|record| record.offset() as usize)
        .ok_or_else(|| "fixture font table is missing".into())
}

fn assert_zero_line_height(bytes: &[u8]) -> TestResult<()> {
    let font = skrifa::FontRef::from_index(bytes, 0)?;
    let location = font.axes().location(std::iter::empty::<(&str, f32)>());
    let metrics = font.metrics(skrifa::prelude::Size::unscaled(), &location);
    assert_eq!((metrics.ascent, metrics.descent), (0.0, 0.0));
    Ok(())
}
