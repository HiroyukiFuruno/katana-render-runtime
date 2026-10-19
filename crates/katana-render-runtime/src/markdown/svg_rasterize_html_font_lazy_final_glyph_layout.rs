use serde::Serialize;

pub(super) const TRANSFORM_COMPONENTS: usize = 6;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FinalGlyphLayout {
    pub(super) id: u32,
    pub(super) text: String,
    pub(super) font_id: String,
    pub(super) font_size: f32,
    pub(super) transform: [f32; TRANSFORM_COMPONENTS],
    pub(super) outline_transform: [f32; TRANSFORM_COMPONENTS],
}

impl FinalGlyphLayout {
    pub(super) fn from_glyph(glyph: &resvg::usvg::layout::PositionedGlyph) -> Result<Self, String> {
        let transform = transform_values(glyph.transform());
        let outline_transform = transform_values(glyph.outline_transform());
        ensure_finite_layout(glyph.id.0, glyph.font_size(), transform, outline_transform)?;
        Ok(Self {
            id: glyph.id.0,
            text: glyph.text.clone(),
            font_id: glyph.font.to_string(),
            font_size: glyph.font_size(),
            transform,
            outline_transform,
        })
    }
}

pub(super) fn transform_values(transform: resvg::usvg::Transform) -> [f32; TRANSFORM_COMPONENTS] {
    [
        transform.sx,
        transform.kx,
        transform.ky,
        transform.sy,
        transform.tx,
        transform.ty,
    ]
}

pub(super) fn transform_is_finite(values: [f32; TRANSFORM_COMPONENTS]) -> bool {
    values.iter().all(|value| value.is_finite())
}

pub(super) fn ensure_finite_layout(
    id: u32,
    font_size: f32,
    transform: [f32; TRANSFORM_COMPONENTS],
    outline_transform: [f32; TRANSFORM_COMPONENTS],
) -> Result<(), String> {
    if !transform_is_finite(transform)
        || !transform_is_finite(outline_transform)
        || !font_size.is_finite()
    {
        return Err(format!("non-finite glyph layout for glyph {id}"));
    }
    Ok(())
}
