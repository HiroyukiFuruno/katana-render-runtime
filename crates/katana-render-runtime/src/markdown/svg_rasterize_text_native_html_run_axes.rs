use super::super::super::font::platform_fallback::actual_fallback::NativeHtmlFallback;
use crate::markdown::svg_rasterize::native_html_faces::OutlineInstance;
use resvg::usvg::FontVariation;
use resvg::usvg::fontdb::FaceInfo;
use skrifa::{FontRef, MetadataProvider, Tag, instance::Size};
use std::collections::BTreeMap;

const AXIS_TAG_BYTES: usize = 4;
const APPLE_HYPERVARIATION_OUTLINE: Tag = Tag::from_be_bytes(*b"hvgl");
const OPTICAL_SIZE_AXIS: Tag = Tag::from_be_bytes(*b"opsz");

pub(super) fn selected_font_axes(
    selection: NativeHtmlFallback,
    face: &FaceInfo,
    bytes: &[u8],
    index: u32,
    character: char,
) -> Option<(Vec<FontVariation>, Option<OutlineInstance>)> {
    let physical = FontRef::from_index(bytes, index).ok()?;
    physical.charmap().map(character)?;
    let selection = if physical.axes().get_by_tag(OPTICAL_SIZE_AXIS).is_some() {
        selection.into_optical_scaler(face, bytes, index)?
    } else {
        selection
    };
    let variations = variation_coordinates(&physical, &selection.explicit_variations)?;
    let outline =
        platform_outline_units(&physical, &variations).map(|units_per_em| OutlineInstance {
            font: selection.font,
            variations: variations.clone(),
            units_per_em,
        });
    Some((variations, outline))
}

fn variation_coordinates(
    font: &FontRef<'_>,
    explicit: &BTreeMap<[u8; AXIS_TAG_BYTES], f64>,
) -> Option<Vec<FontVariation>> {
    let axes = font.axes();
    if explicit
        .keys()
        .any(|tag| axes.get_by_tag(Tag::from_be_bytes(*tag)).is_none())
    {
        return None;
    }
    axes.iter()
        .map(|axis| {
            let tag = axis.tag().to_be_bytes();
            let value = explicit
                .get(&tag)
                .copied()
                .unwrap_or_else(|| f64::from(axis.default_value()));
            (value >= f64::from(axis.min_value()) && value <= f64::from(axis.max_value()))
                .then_some(FontVariation::new(tag, value as f32))
        })
        .collect()
}

fn platform_outline_units(font: &FontRef<'_>, variations: &[FontVariation]) -> Option<u16> {
    if font.outline_glyphs().format().is_some() {
        return None;
    }
    /* WHY: Apple固有のhvgl字形だけCoreTextに委譲し、SVGエンジンと同じ単位で受け取る。 */
    font.table_data(APPLE_HYPERVARIATION_OUTLINE)?;
    let location = font.axes().location(
        variations
            .iter()
            .map(|v| (Tag::from_be_bytes(v.tag), v.value)),
    );
    Some(font.metrics(Size::unscaled(), &location).units_per_em)
}
