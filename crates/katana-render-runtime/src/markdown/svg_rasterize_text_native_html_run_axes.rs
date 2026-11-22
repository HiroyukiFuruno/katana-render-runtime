use resvg::usvg::FontVariation;
use skrifa::{FontRef, MetadataProvider, Tag, instance::Size};
use std::collections::BTreeMap;

const AXIS_TAG_BYTES: usize = 4;
const APPLE_HYPERVARIATION_OUTLINE: Tag = Tag::from_be_bytes(*b"hvgl");

pub(super) fn variation_coordinates(
    bytes: &[u8],
    index: u32,
    ch: char,
    explicit: &BTreeMap<[u8; AXIS_TAG_BYTES], f64>,
) -> Option<(Vec<FontVariation>, Option<u16>)> {
    let font = FontRef::from_index(bytes, index).ok()?;
    font.charmap().map(ch)?;
    let axes = font.axes();
    if explicit
        .keys()
        .any(|tag| axes.get_by_tag(Tag::from_be_bytes(*tag)).is_none())
    {
        return None;
    }
    let variations: Vec<_> = axes
        .iter()
        .map(|axis| {
            let tag = axis.tag().to_be_bytes();
            let value = explicit
                .get(&tag)
                .copied()
                .unwrap_or_else(|| f64::from(axis.default_value()));
            (value >= f64::from(axis.min_value()) && value <= f64::from(axis.max_value()))
                .then_some(FontVariation::new(tag, value as f32))
        })
        .collect::<Option<_>>()?;
    let outline_units = platform_outline_units(&font, &variations);
    Some((variations, outline_units))
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
