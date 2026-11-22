use objc2_core_foundation::CFRetained;
use objc2_core_text::CTFont;
use resvg::usvg::FontVariation;

#[path = "svg_rasterize_native_html_face_outline.rs"]
mod coretext;

const MAX_INSTANCES_PER_FACE: usize = 32;

pub(in crate::markdown::svg_rasterize) struct OutlineInstance {
    pub(in crate::markdown::svg_rasterize) font: CFRetained<CTFont>,
    pub(in crate::markdown::svg_rasterize) variations: Vec<FontVariation>,
    pub(in crate::markdown::svg_rasterize) units_per_em: u16,
}

#[derive(Default)]
pub(super) struct OutlineInstances(Vec<OutlineInstance>);

impl OutlineInstances {
    pub(super) fn insert(&mut self, instance: OutlineInstance) -> Option<()> {
        if self.find(&instance.variations).is_some() {
            return Some(());
        }
        (self.0.len() < MAX_INSTANCES_PER_FACE).then_some(())?;
        self.0.push(instance);
        Some(())
    }

    pub(super) fn path(
        &self,
        glyph_id: u16,
        coordinates: &[FontVariation],
    ) -> Option<tiny_skia_path::Path> {
        let instance = self.find(coordinates)?;
        coretext::outline(&instance.font, glyph_id, instance.units_per_em)
    }

    fn find(&self, coordinates: &[FontVariation]) -> Option<&OutlineInstance> {
        self.0
            .iter()
            .find(|instance| same_variations(&instance.variations, coordinates))
    }
}

pub(in crate::markdown::svg_rasterize) fn same_variations(
    left: &[FontVariation],
    right: &[FontVariation],
) -> bool {
    let coordinates = |variation: &FontVariation| (variation.tag, variation.value.to_bits());
    left.iter()
        .map(coordinates)
        .eq(right.iter().map(coordinates))
}
