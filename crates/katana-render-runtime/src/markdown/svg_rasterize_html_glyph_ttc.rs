use std::ops::Range;

use skia_safe::{Data, FontMgr, Typeface};
use write_fonts::{
    FontBuilder,
    read::{FontRef as ReadFontRef, TableProvider},
    types::Tag,
};

const HEAD_CHECKSUM_RANGE: Range<usize> = 8..12;

pub(super) fn repack_face(bytes: &[u8], index: u32) -> Result<Vec<u8>, String> {
    let source = ReadFontRef::from_index(bytes, index)
        .map_err(|error| format!("TTC face {index} cannot be read: {error:?}"))?;
    let output = FontBuilder::new().copy_missing_tables(source).build();
    verify_face(bytes, index, &output)?;
    Ok(output)
}

pub(super) fn create_physical_typeface(
    data: &Data,
    index: u32,
    font_mgr: &FontMgr,
) -> Result<Option<Typeface>, String> {
    if let Some(typeface) = font_mgr.new_from_data(data.clone(), Some(index)) {
        return Ok(Some(typeface));
    }
    if index == 0 {
        return Ok(None);
    }
    let repacked = repack_face(data.as_bytes(), index)?;
    let repacked_data = Data::new_copy(&repacked);
    Ok(font_mgr.new_from_data(repacked_data, Some(0)))
}

pub(super) fn verify_face(
    source_bytes: &[u8],
    source_index: u32,
    output: &[u8],
) -> Result<(), String> {
    let source = ReadFontRef::from_index(source_bytes, source_index)
        .map_err(|error| format!("source TTC face verification failed: {error:?}"))?;
    let output = ReadFontRef::from_index(output, 0)
        .map_err(|error| format!("repacked SFNT verification failed: {error:?}"))?;
    compare_tables(&source, &output)?;
    validate_metrics(&source, &output)?;
    Ok(())
}

fn validate_metrics(source: &ReadFontRef<'_>, output: &ReadFontRef<'_>) -> Result<(), String> {
    let source_head = source
        .head()
        .map_err(|error| format!("source head table missing: {error:?}"))?;
    let output_head = output
        .head()
        .map_err(|error| format!("repacked head table missing: {error:?}"))?;
    if source_head.units_per_em() != output_head.units_per_em() {
        return Err("repacked face units-per-em changed".to_string());
    }
    let source_glyphs = source
        .maxp()
        .map_err(|error| format!("source maxp table missing: {error:?}"))?
        .num_glyphs();
    let output_glyphs = output
        .maxp()
        .map_err(|error| format!("repacked maxp table missing: {error:?}"))?
        .num_glyphs();
    if source_glyphs != output_glyphs {
        return Err("repacked face glyph count changed".to_string());
    }
    Ok(())
}

fn compare_tables(source: &ReadFontRef<'_>, output: &ReadFontRef<'_>) -> Result<(), String> {
    let source_tables = source.table_directory().table_records();
    let output_tables = output.table_directory().table_records();
    let mut source_tags = source_tables
        .iter()
        .map(|record| record.tag())
        .collect::<Vec<_>>();
    let mut output_tags = output_tables
        .iter()
        .map(|record| record.tag())
        .collect::<Vec<_>>();
    compare_tag_sets(&mut source_tags, &mut output_tags)?;
    for record in source_tables.iter() {
        compare_table_data(source, output, record.tag())?;
    }
    Ok(())
}

fn compare_tag_sets(source_tags: &mut [Tag], output_tags: &mut [Tag]) -> Result<(), String> {
    source_tags.sort();
    output_tags.sort();
    if source_tags != output_tags {
        return Err("repacked face table tag set changed".to_string());
    }
    Ok(())
}

fn compare_table_data(
    source: &ReadFontRef<'_>,
    output: &ReadFontRef<'_>,
    tag: Tag,
) -> Result<(), String> {
    let source_data = source
        .data_for_tag(tag)
        .ok_or("source table data missing")?;
    let output_data = output
        .data_for_tag(tag)
        .ok_or("repacked table data missing")?;
    if !table_bytes_equal(tag, source_data.as_bytes(), output_data.as_bytes()) {
        return Err(format!("repacked table {tag:?} changed"));
    }
    Ok(())
}

fn table_bytes_equal(tag: Tag, source: &[u8], output: &[u8]) -> bool {
    if tag != Tag::new(b"head")
        || source.len() < HEAD_CHECKSUM_RANGE.end
        || output.len() < HEAD_CHECKSUM_RANGE.end
    {
        return source == output;
    }
    let mut source = source.to_vec();
    let mut output = output.to_vec();
    source[HEAD_CHECKSUM_RANGE.clone()].fill(0);
    output[HEAD_CHECKSUM_RANGE.clone()].fill(0);
    source == output
}

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_ttc_verification_support.rs"]
pub(super) mod verification_support;

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_ttc_verification_tests.rs"]
mod verification_tests;

#[cfg(test)]
mod tests {
    use super::super::font_file_fixture;
    use super::{
        HEAD_CHECKSUM_RANGE, create_physical_typeface, repack_face, table_bytes_equal, verify_face,
    };
    use skia_safe::{Data, FontMgr};
    use write_fonts::types::Tag;

    #[test]
    fn rejects_invalid_face_index() {
        assert!(repack_face(font_file_fixture::FONT_BYTES, u32::MAX).is_err());
    }

    #[test]
    fn rejects_corrupt_source() {
        let mut bytes = font_file_fixture::FONT_BYTES.to_vec();
        bytes[0] ^= 0xff;
        assert!(repack_face(&bytes, 0).is_err());
    }

    #[test]
    fn missing_primary_face_returns_none() {
        let data = Data::new_copy(b"not a font");
        assert!(matches!(
            create_physical_typeface(&data, 0, &FontMgr::default()),
            Ok(None)
        ));
    }

    #[test]
    fn missing_collection_face_reports_repack_error() {
        let data = Data::new_copy(b"not a font");
        let result = create_physical_typeface(&data, 1, &FontMgr::default());
        assert!(matches!(result, Err(error) if error.contains("TTC face 1 cannot be read")));
    }

    #[test]
    fn rejects_corrupt_repacked_output() {
        let result = verify_face(font_file_fixture::FONT_BYTES, 0, b"not a font");
        assert!(
            matches!(result, Err(error) if error.contains("repacked SFNT verification failed"))
        );
    }

    #[test]
    fn rejects_non_head_table_mismatch() {
        let tag = Tag::new(b"cmap");
        assert!(!table_bytes_equal(tag, &[1, 2], &[1, 3]));
    }

    #[test]
    fn permits_head_checksum_repacking_difference() {
        let tag = Tag::new(b"head");
        let mut source = [0u8; HEAD_CHECKSUM_RANGE.end];
        let mut output = source;
        output[HEAD_CHECKSUM_RANGE.start] = 1;
        assert!(table_bytes_equal(tag, &source, &output));
        source[0] = 1;
        assert!(!table_bytes_equal(tag, &source, &output));
    }
}
