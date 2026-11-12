use super::{
    CBDT_FONT, TestGlyph, TestResult, bitmap_options, fixture_glyph, render_clip_fixture, usvg,
};
use crate::svg_engine::usvg::flatten::DatabaseExt;

#[test]
fn one_bit_bitmap_glyph_does_not_paint_or_hide_siblings() -> TestResult<()> {
    let bytes = one_bit_bitmap_fixture()?;
    let glyph = fixture_glyph(&bytes, TestGlyph::Character('😀'))?;
    assert_one_bit_bitmap(&bytes, glyph)?;
    let options = bitmap_options(&bytes)?;
    let face = options.fontdb.faces().next().ok_or("bitmap face missing")?;
    assert!(options.fontdb.raster(face.id, glyph).is_none());
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32">
      <rect width="4" height="4" fill="blue"/>
      <text id="label" x="12" y="24" font-size="16">😀</text></svg>"#;
    let png_tree = usvg::Tree::from_str(source, &bitmap_options(CBDT_FONT)?)
        .map_err(|error| error.to_string())?;
    assert!(matches!(
        png_tree.node_by_id("label"),
        Some(usvg::Node::Text(_))
    ));
    let tree = usvg::Tree::from_str(source, &options).map_err(|error| error.to_string())?;
    assert!(tree.node_by_id("label").is_none());
    let actual = render_clip_fixture(&tree)?;
    assert_eq!(
        actual
            .pixels()
            .iter()
            .filter(|pixel| pixel.alpha() > 0)
            .count(),
        16
    );
    assert_eq!(actual.pixel(1, 1).map(|pixel| pixel.blue()), Some(255));
    Ok(())
}

fn assert_one_bit_bitmap(bytes: &[u8], glyph: usvg::GlyphId) -> TestResult<()> {
    use skrifa::{MetadataProvider, bitmap::BitmapData, instance::Size};
    let font = skrifa::FontRef::new(bytes).map_err(|error| error.to_string())?;
    let bitmap = font
        .bitmap_strikes()
        .glyph_for_size(Size::unscaled(), glyph.into())
        .ok_or("bitmap strike missing")?;
    assert!(matches!(bitmap.data, BitmapData::Mask(mask) if mask.bpp == 1));
    Ok(())
}

fn one_bit_bitmap_fixture() -> TestResult<Vec<u8>> {
    use rustybuzz::ttf_parser::{RawFace, Tag};
    const DATA_TAG_OFFSET: usize = 12;
    const INDEX_TAG_OFFSET: usize = DATA_TAG_OFFSET + 16;
    let font = RawFace::parse(CBDT_FONT, 0).map_err(|error| format!("{error:?}"))?;
    let tables = font.table_records;
    let data = tables.get(0).ok_or("bitmap data table missing")?;
    let index = tables.get(1).ok_or("bitmap index table missing")?;
    assert_eq!(data.tag, Tag::from_bytes(b"CBDT"));
    assert_eq!(index.tag, Tag::from_bytes(b"CBLC"));
    let data = data.offset as usize;
    let index = index.offset as usize;
    let mut bytes = CBDT_FONT.to_vec();
    write_fixture_field(&mut bytes, DATA_TAG_OFFSET, b"EBDT");
    write_fixture_field(&mut bytes, INDEX_TAG_OFFSET, b"EBLC");
    write_bitmap_tables(&mut bytes, data, index);
    Ok(bytes)
}

fn write_bitmap_tables(bytes: &mut [u8], data: usize, index: usize) {
    const BIT_DEPTH_OFFSET: usize = 54;
    const IMAGE_FORMAT_OFFSET: usize = 66;
    const FINAL_IMAGE_OFFSET: usize = 76;
    const BITMAP_TABLE_VERSION: [u8; 4] = [0, 2, 0, 0];
    const BYTE_ALIGNED_FORMAT: u16 = 1;
    const FIRST_BIT_SET: u8 = 0x80;
    const GLYPH: [u8; 6] = [1, 1, 0, 1, 1, FIRST_BIT_SET];
    write_fixture_field(bytes, data, &BITMAP_TABLE_VERSION);
    write_fixture_field(bytes, index, &BITMAP_TABLE_VERSION);
    bytes[index + BIT_DEPTH_OFFSET] = 1;
    write_fixture_field(
        bytes,
        index + IMAGE_FORMAT_OFFSET,
        &BYTE_ALIGNED_FORMAT.to_be_bytes(),
    );
    write_fixture_field(
        bytes,
        index + FINAL_IMAGE_OFFSET,
        &(GLYPH.len() as u32).to_be_bytes(),
    );
    /* WHY: 1×1の小メトリクスに、左上1ビットが立った1バイトのマスクを置く。 */
    write_fixture_field(bytes, data + BITMAP_TABLE_VERSION.len(), &GLYPH);
}

fn write_fixture_field(bytes: &mut [u8], offset: usize, value: &[u8]) {
    bytes[offset..offset + value.len()].copy_from_slice(value);
}
