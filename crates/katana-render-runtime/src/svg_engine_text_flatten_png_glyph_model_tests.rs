use super::png_glyph_image;
use skrifa::MetadataProvider;
use skrifa::bitmap::{BitmapData, BitmapGlyph};

const CBDT_FONT: &[u8] = include_bytes!(
    "../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/NotoColorEmojiCBDT.subset.ttf"
);
const SBIX_FONT: &[u8] = include_bytes!(
    "../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/CFF-and-SBIX.otf"
);
const CBDT_CHARACTER: char = '😀';
const SBIX_CHARACTER: char = 'A';
const ZERO_DIMENSION: u32 = 0;

#[test]
fn cbdt_and_sbix_png_glyphs_reject_zero_dimensions() -> Result<(), String> {
    for (bytes, character) in [(CBDT_FONT, CBDT_CHARACTER), (SBIX_FONT, SBIX_CHARACTER)] {
        assert_png_glyph_zero_dimensions_rejected(bytes, character)?;
    }
    Ok(())
}

fn assert_png_glyph_zero_dimensions_rejected(bytes: &[u8], character: char) -> Result<(), String> {
    let face = rustybuzz::ttf_parser::Face::parse(bytes, 0)
        .map_err(|error| format!("bitmap fixture parse failed: {error:?}"))?;
    let glyph = face
        .glyph_index(character)
        .ok_or("bitmap fixture glyph missing")?;
    let glyph_id = crate::svg_engine::usvg::GlyphId(u32::from(glyph.0));
    let font = skrifa::FontRef::from_index(bytes, 0).map_err(|error| format!("{error:?}"))?;
    let bitmap = font
        .bitmap_strikes()
        .glyph_for_size(skrifa::prelude::Size::unscaled(), glyph_id.into())
        .ok_or("bitmap glyph missing")?;
    let BitmapData::Png(data) = &bitmap.data else {
        return Err("bitmap fixture glyph is not PNG data".to_owned());
    };
    assert!(png_glyph_image(&bitmap, data).is_some());
    for (width, height) in [
        (ZERO_DIMENSION, bitmap.height),
        (bitmap.width, ZERO_DIMENSION),
    ] {
        let invalid = BitmapGlyph {
            width,
            height,
            ..bitmap.clone()
        };
        assert!(png_glyph_image(&invalid, data).is_none());
    }
    Ok(())
}
