use super::{Face, character_metrics};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
use read_fonts::FontRef;
use read_fonts::types::Tag;

const OS2_AVERAGE_CHAR_WIDTH_OFFSET: usize = 2;
const INPUT_FONT_SIZE: f32 = 16.0;
const NARROW_INPUT_SIZE: u16 = 4;
const WIDE_INPUT_SIZE: u16 = 12;

#[test]
fn linux_input_metrics_reject_an_unresolvable_font_family() {
    assert!(
        SvgRasterizeOps::html_input_character_metrics(
            "KRR Unavailable Input Font 7f91",
            INPUT_FONT_SIZE,
            400,
            false,
        )
        .is_none()
    );
}

#[test]
fn linux_input_metrics_use_real_primary_font_measurements() -> Result<(), String> {
    let metrics = SvgRasterizeOps::html_input_character_metrics(
        "Noto Sans, sans-serif",
        INPUT_FONT_SIZE,
        400,
        false,
    )
    .ok_or("Noto Sans input metrics unavailable")?;

    let narrow = metrics.width_for_size(usize::from(NARROW_INPUT_SIZE));
    let wide = metrics.width_for_size(usize::from(WIDE_INPUT_SIZE));
    assert!(narrow.is_finite() && narrow > 0.0);
    assert!(wide > narrow);
    Ok(())
}

#[test]
fn linux_zero_os2_average_uses_zero_glyph_width_without_maximum_padding() -> Result<(), String> {
    let mut bytes = FONT_BYTES.to_vec();
    let offset = table_offset(FONT_BYTES, Tag::new(b"OS/2"))? + OS2_AVERAGE_CHAR_WIDTH_OFFSET;
    bytes
        .get_mut(offset..offset + 2)
        .ok_or("OS/2 average character width is absent")?
        .copy_from_slice(&0_i16.to_be_bytes());
    let face = Face::parse(&bytes, 0).map_err(|_| "zero-average fixture is invalid")?;
    let metrics = character_metrics(&face, 1.0).ok_or("zero-average metrics unavailable")?;
    let zero = f32::from(
        face.glyph_hor_advance(face.glyph_index('0').ok_or("zero glyph missing")?)
            .ok_or("zero glyph advance missing")?,
    );

    for size in [NARROW_INPUT_SIZE, WIDE_INPUT_SIZE] {
        assert_eq!(
            metrics.width_for_size(usize::from(size)),
            (zero * f32::from(size)).ceil()
        );
    }
    Ok(())
}

fn table_offset(data: &[u8], tag: Tag) -> Result<usize, String> {
    let font = FontRef::new(data).map_err(|error| error.to_string())?;
    font.table_directory()
        .table_records()
        .iter()
        .find(|record| record.tag() == tag)
        .map(|record| record.offset() as usize)
        .ok_or_else(|| format!("missing {tag} table"))
}
