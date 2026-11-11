use super::{
    FontShape, SvgTextRunMetrics, harfrust, html_font_db_for_measured_text, matching_font_face,
    resolved_font_runs, svg_metrics,
};
use crate::markdown::svg_rasterize::SvgRasterizeError;
use std::sync::Arc;

#[cfg(test)]
#[path = "svg_rasterize_text_shaping_svg_fragments_tests.rs"]
mod tests;

pub(crate) struct SvgTextFragmentLayout {
    pub(crate) metrics: SvgTextRunMetrics,
    pub(crate) advances: Vec<f32>,
}

pub(in crate::markdown::svg_rasterize) fn shaped_svg_fragment_layout(
    fragments: &[String],
    family: &str,
    size: f32,
    weight: u16,
    italic: bool,
) -> Result<Option<SvgTextFragmentLayout>, SvgRasterizeError> {
    if fragments.is_empty()
        || fragments
            .iter()
            .any(|fragment| fragment.is_empty() || fragment.contains('\0'))
    {
        return Ok(None);
    }
    let Some(font) = svg_metrics::validated_svg_font_shape(size, weight, italic) else {
        return Ok(None);
    };
    let text = fragments.concat();
    if !supported_scalars(&text) || unicode_bidi::BidiInfo::new(&text, None).has_rtl() {
        return Ok(None);
    }
    layout_for_text(&text, fragments, family, font)
}

fn layout_for_text(
    text: &str,
    fragments: &[String],
    family: &str,
    font: FontShape,
) -> Result<Option<SvgTextFragmentLayout>, SvgRasterizeError> {
    let database = html_font_db_for_measured_text(family, text, font.weight, font.italic);
    layout_in_database(&database, text, fragments, family, font)
}

fn layout_in_database(
    database: &Arc<resvg::usvg::fontdb::Database>,
    text: &str,
    fragments: &[String],
    family: &str,
    font: FontShape,
) -> Result<Option<SvgTextFragmentLayout>, SvgRasterizeError> {
    let base = matching_font_face(database, family, font.weight, font.italic)
        .ok_or_else(|| error("SVG font face unavailable"))?;
    let runs = resolved_font_runs(database, base, family, text, font.weight, font.italic);
    let [(face, _)] = runs.as_slice() else {
        return Ok(None);
    };
    if !fragment_faces_match(database, base, *face, family, font, fragments) {
        return Ok(None);
    }
    layout_for_face(database, base, *face, text, font, fragments)
}

fn fragment_faces_match(
    database: &Arc<resvg::usvg::fontdb::Database>,
    base: resvg::usvg::fontdb::ID,
    face: resvg::usvg::fontdb::ID,
    family: &str,
    font: FontShape,
    fragments: &[String],
) -> bool {
    fragments.iter().all(|fragment| {
        let runs = resolved_font_runs(database, base, family, fragment, font.weight, font.italic);
        matches!(runs.as_slice(), [(id, _)] if *id == face)
    })
}

fn supported_scalars(text: &str) -> bool {
    use unicode_bidi::BidiClass::{CS, EN, ES, ET, L, ON, WS};
    text.chars().all(|character| {
        !character.is_control()
            && matches!(
                unicode_bidi::bidi_class(character),
                L | EN | ES | ET | CS | ON | WS
            )
    })
}

fn layout_for_face(
    database: &resvg::usvg::fontdb::Database,
    base: resvg::usvg::fontdb::ID,
    face: resvg::usvg::fontdb::ID,
    text: &str,
    font: FontShape,
    fragments: &[String],
) -> Result<Option<SvgTextFragmentLayout>, SvgRasterizeError> {
    let shaped = harfrust::shape_fragment_run(database, face, text, font)
        .map_err(|message| error(&message))?;
    let Some((mut metrics, characters)) = shaped else {
        return Ok(None);
    };
    metrics.layout_ascent =
        svg_metrics::svg_dom_layout_ascent(database.face(face), metrics.ascent, metrics.descent);
    svg_metrics::include_svg_primary_strut(database, base, font, face == base, &mut metrics)
        .ok_or_else(|| error("SVG primary font metrics unavailable"))?;
    let metrics = svg_metrics::validated_svg_run_metrics(metrics)
        .ok_or_else(|| error("invalid SVG metrics"))?;
    if !ink_inside_advance(&metrics) {
        return Ok(None);
    }
    let Some(advances) = fragment_advances(fragments, &characters) else {
        return Ok(None);
    };
    if !rounded_advances_cover_full(&advances, metrics.advance) {
        return Ok(None);
    }
    Ok(Some(SvgTextFragmentLayout { metrics, advances }))
}

fn rounded_advances_cover_full(advances: &[f32], full: f32) -> bool {
    const LAYOUT_UNITS: f64 = 64.0;
    advances
        .iter()
        .map(|value| (f64::from(*value) * LAYOUT_UNITS).ceil() / LAYOUT_UNITS)
        .sum::<f64>()
        >= f64::from(full)
}

fn ink_inside_advance(metrics: &SvgTextRunMetrics) -> bool {
    metrics.ink_bounds.is_none_or(|bounds| {
        bounds.iter().all(|value| value.is_finite())
            && bounds[0] >= 0.0
            && bounds[2] <= f64::from(metrics.advance)
    })
}

fn fragment_advances(fragments: &[String], characters: &[f32]) -> Option<Vec<f32>> {
    let mut offset = 0_usize;
    let mut result = Vec::with_capacity(fragments.len());
    for fragment in fragments {
        let end = offset.checked_add(fragment.chars().count())?;
        let advance = characters.get(offset..end)?.iter().sum::<f32>();
        if !advance.is_finite() || advance < 0.0 {
            return None;
        }
        result.push(advance);
        offset = end;
    }
    (offset == characters.len()).then_some(result)
}

fn error(message: &str) -> SvgRasterizeError {
    SvgRasterizeError::RasterizeFailed(message.to_owned())
}
