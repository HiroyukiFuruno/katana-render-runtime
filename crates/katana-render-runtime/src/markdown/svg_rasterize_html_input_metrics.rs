use super::{html_font_db_for_measured_text, matching_font_face};
use rustybuzz::ttf_parser::{Face, Tag};

#[cfg(not(target_os = "macos"))]
const OS2_AVERAGE_CHAR_WIDTH_BYTES: std::ops::Range<usize> = 2..4;
#[cfg(target_os = "windows")]
const HHEA_MAX_ADVANCE_BYTES: std::ops::Range<usize> = 10..12;

pub(crate) struct InputCharacterMetrics {
    average: f32,
    maximum: f32,
    #[cfg(not(target_os = "macos"))]
    average_from_primary_font: bool,
}

impl InputCharacterMetrics {
    pub(crate) fn width_for_size(&self, size: usize) -> f32 {
        #[cfg(not(target_os = "macos"))]
        if !self.average_from_primary_font {
            return (self.average * size as f32).ceil();
        }
        let average = self.average.round();
        (average * size as f32).ceil() + self.maximum.round() - average
    }
}

pub(in super::super) fn html_input_character_metrics(
    family: &str,
    size: f32,
    weight: u16,
    italic: bool,
) -> Option<InputCharacterMetrics> {
    let size = super::effective_html_font_size(size);
    let database = html_font_db_for_measured_text(family, "x0", weight, italic);
    let id = matching_font_face(&database, family, weight, italic)?;
    database.with_face_data(id, |data, index| {
        let mut face = Face::parse(data, index).ok()?;
        for (tag, value) in [
            (Tag::from_bytes(b"wght"), f32::from(weight)),
            (Tag::from_bytes(b"ital"), if italic { 1.0 } else { 0.0 }),
            (Tag::from_bytes(b"opsz"), size),
        ] {
            let _ = face.set_variation(tag, value);
        }
        character_metrics(&face, size / f32::from(face.units_per_em()))
    })?
}

#[cfg(target_os = "macos")]
fn character_metrics(face: &Face<'_>, scale: f32) -> Option<InputCharacterMetrics> {
    /* WHY: BlinkのmacOS入力幅は平均をx glyph、最大をprimary fontのascentから求める。 */
    let average = f32::from(face.glyph_hor_advance(face.glyph_index('x')?)?) * scale;
    Some(InputCharacterMetrics {
        average,
        maximum: f32::from(face.ascender()) * scale,
    })
}

#[cfg(not(target_os = "macos"))]
fn character_metrics(face: &Face<'_>, scale: f32) -> Option<InputCharacterMetrics> {
    let table = face.raw_face().table(Tag::from_bytes(b"OS/2"))?;
    let average = f32::from(i16::from_be_bytes(
        table.get(OS2_AVERAGE_CHAR_WIDTH_BYTES)?.try_into().ok()?,
    )) * scale;
    if average <= 0.0 {
        let zero = f32::from(face.glyph_hor_advance(face.glyph_index('0')?)?) * scale;
        return Some(InputCharacterMetrics {
            average: zero,
            maximum: zero,
            average_from_primary_font: false,
        });
    }
    Some(InputCharacterMetrics {
        average,
        maximum: maximum_character_width(face, scale)?,
        average_from_primary_font: true,
    })
}

#[cfg(all(not(target_os = "macos"), target_os = "windows"))]
fn maximum_character_width(face: &Face<'_>, scale: f32) -> Option<f32> {
    let table = face.raw_face().table(Tag::from_bytes(b"hhea"))?;
    Some(
        f32::from(u16::from_be_bytes(
            table.get(HHEA_MAX_ADVANCE_BYTES)?.try_into().ok()?,
        )) * scale,
    )
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn maximum_character_width(face: &Face<'_>, scale: f32) -> Option<f32> {
    let bounds = face.global_bounding_box();
    Some((f32::from(bounds.x_max) - f32::from(bounds.x_min)) * scale)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::{Face, Tag, character_metrics, html_input_character_metrics};
    use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;

    #[test]
    fn input_metrics_decline_a_font_family_that_cannot_be_resolved() {
        assert!(html_input_character_metrics("Noto Sans", 16.0, 400, false).is_some());
        assert!(
            html_input_character_metrics("KRR Unavailable Input Font 7f91", 16.0, 400, false)
                .is_none()
        );
    }

    #[test]
    fn input_metrics_reject_a_real_font_without_x_glyph_or_advance()
    -> Result<(), Box<dyn std::error::Error>> {
        let original = Face::parse(FONT_BYTES, 0).map_err(|_| "bundled font is invalid")?;
        assert!(character_metrics(&original, 1.0).is_some());

        let subset = super::super::fallback::font_with_only_b_cmap(FONT_BYTES)?;
        let face = Face::parse(&subset, 0).map_err(|_| "subset font is invalid")?;
        assert!(face.glyph_index('B').is_some());
        assert!(character_metrics(&face, 1.0).is_none());

        let without_advance = super::super::test_font_data::font_with_empty_table(
            FONT_BYTES,
            Tag::from_bytes(b"hmtx"),
        )?;
        let face = Face::parse(&without_advance, 0).map_err(|_| "fixture font is invalid")?;
        assert!(face.glyph_index('x').is_some());
        assert!(character_metrics(&face, 1.0).is_none());
        Ok(())
    }

    #[test]
    fn fractional_font_size_uses_chrome_font_cache_precision() {
        use crate::markdown::svg_rasterize::SvgRasterizeOps;
        assert!(
            SvgRasterizeOps::measure_html_text("x", "Arial", 13.333333, 400, false, 0.0, None)
                .is_ok_and(|width| {
                    assert!((width - 6.665).abs() < 0.0001, "width {width}");
                    true
                }),
            "Arial fixture text measurement must succeed"
        );
    }

    #[test]
    fn arial_input_size_matches_independent_chrome_width() {
        assert!(
            html_input_character_metrics("Arial", 13.333333, 400, false)
                .is_some_and(|metrics| metrics.width_for_size(20) == 145.0),
            "Arial fixture input width must match Chrome at 145px"
        );
    }
}
