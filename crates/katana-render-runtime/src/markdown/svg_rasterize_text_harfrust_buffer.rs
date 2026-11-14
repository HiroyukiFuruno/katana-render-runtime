use super::super::features::harfrust_features;
use super::{FONT_VARIATION_COUNT, FontShape, HARFRUST_SCALE};

pub(super) fn shape_font(
    face: &harfrust::font::Font,
    text: &str,
    font: FontShape,
    font_feature_settings: Option<&str>,
) -> Option<harfrust::Buffer> {
    let instance = face
        .instance_builder()
        .variations(font_variations(font))
        .build();
    shape_buffer(&instance, text, font, font_feature_settings)
}

pub(in crate::markdown::svg_rasterize::text_shaping) fn shape_buffer_with_features(
    instance: &harfrust::font::Font,
    text: &str,
    font: FontShape,
    features: &[harfrust::Feature],
) -> Option<harfrust::Buffer> {
    shape_buffer_features(instance, text, font, features)
}

pub(super) fn shape_font_with_features(
    face: &harfrust::font::Font,
    text: &str,
    font: FontShape,
    features: &[harfrust::Feature],
) -> Option<harfrust::Buffer> {
    let instance = face
        .instance_builder()
        .variations(font_variations(font))
        .build();
    shape_buffer_with_features(&instance, text, font, features)
}

pub(super) fn is_left_to_right(text: &str) -> bool {
    let mut buffer = harfrust::Buffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    buffer.direction() == harfrust::Direction::LeftToRight
}

pub(super) fn font_variations(font: FontShape) -> [(&'static str, f32); FONT_VARIATION_COUNT] {
    [
        ("wght", f32::from(font.weight)),
        ("opsz", font.size),
        ("ital", if font.italic { 1.0 } else { 0.0 }),
    ]
}

pub(super) fn shape_buffer(
    face: &harfrust::font::Font,
    text: &str,
    font: FontShape,
    font_feature_settings: Option<&str>,
) -> Option<harfrust::Buffer> {
    let features = harfrust_features(font_feature_settings);
    let instance = face
        .instance_builder()
        .variations(font_variations(font))
        .build();
    shape_buffer_features(&instance, text, font, &features)
}

fn shape_buffer_features(
    instance: &harfrust::font::Font,
    text: &str,
    font: FontShape,
    features: &[harfrust::Feature],
) -> Option<harfrust::Buffer> {
    use harfrust::{Buffer, ShapeOptions, ShaperFont};
    let shaper = ShaperFont::new(instance).with_scale((font.size * HARFRUST_SCALE) as i32);
    let mut buffer = Buffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    harfrust::shape(
        &shaper,
        &mut buffer,
        ShapeOptions::new()
            .point_size(Some(font.size))
            .features(features),
    )
    .ok()?;
    Some(buffer)
}
