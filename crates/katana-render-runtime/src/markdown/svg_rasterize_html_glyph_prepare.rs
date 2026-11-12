use super::{
    HtmlGlyphRasterizer, Prepared, PreparedSpan, SpanInput, XAlignment, anchor, baseline, chunk,
    decorations, fallback_runs, multi_chunk_alignment, runs, single_horizontal_alignment,
};
use resvg::usvg;

impl HtmlGlyphRasterizer<'_> {
    pub(super) fn prepare(&self, text: &usvg::Text) -> Result<Prepared, String> {
        let policy = baseline::resolved_policy(self, text)?;
        let has_effects = text.layouted().iter().any(decorations::has_effects);
        if has_effects && text.layouted().len() != 1 {
            return Ok(Prepared::Unsupported);
        }
        let mut prepared = Vec::new();
        for span in text
            .layouted()
            .iter()
            .filter(|span| !span.positioned_glyphs.is_empty())
        {
            if !self.prepare_span(span, policy, &mut prepared)? {
                return Ok(Prepared::Unsupported);
            }
        }
        if has_effects && (prepared.len() != 1 || prepared[0].baseline_shift != 0.0) {
            return Ok(Prepared::Unsupported);
        }
        if !has_effects || !text.layouted().iter().any(decorations::has_decorations) {
            self.apply_horizontal_alignment(text, &mut prepared)?;
        }
        Ok(Prepared::Ready(prepared))
    }

    fn apply_horizontal_alignment(
        &self,
        text: &usvg::Text,
        prepared: &mut [PreparedSpan],
    ) -> Result<(), String> {
        if self.apply_fallback_alignment(text, prepared)? {
            return Ok(());
        }
        if chunk::apply_single_middle(text, prepared)? {
            return Ok(());
        }
        if let [span] = prepared
            && let Some(anchor_x) = single_horizontal_alignment(text, span)
        {
            span.x_alignment = Some(XAlignment {
                anchor_x,
                layout: anchor::layout_anchor(text),
                origin_snap_css_scale: anchor::run_origin(text, std::slice::from_ref(span))
                    .map(|(scale, _)| scale),
            });
            return Ok(());
        }
        if multi_chunk_alignment::apply(text, prepared) {
            return Ok(());
        }
        chunk::apply(self.database, text, prepared)
    }

    fn apply_fallback_alignment(
        &self,
        text: &usvg::Text,
        prepared: &mut [PreparedSpan],
    ) -> Result<bool, String> {
        /* WHY: fallback runも実faceのCSSサイズで測り、名目SVGサイズの文字間隔を残さない。 */
        let Some(layouts) = fallback_runs::layout(self.database, text, prepared)? else {
            return Ok(false);
        };
        for (span, layout) in prepared.iter_mut().zip(layouts) {
            span.fallback_alignment = Some(layout);
        }
        Ok(true)
    }

    pub(super) fn prepare_span(
        &self,
        span: &usvg::layout::Span,
        policy: Option<baseline::Policy>,
        prepared: &mut Vec<PreparedSpan>,
    ) -> Result<bool, String> {
        if !span_supported(span) {
            return Ok(false);
        }
        let Some(mut effects) = prepare_effects(span)? else {
            return Ok(false);
        };
        let Some((color, opacity)) = solid_fill(span) else {
            return Ok(false);
        };
        if !effects_runs_supported(span, effects.is_some()) {
            return Ok(false);
        }
        for run in runs::consecutive_runs(&span.positioned_glyphs) {
            let glyphs = &span.positioned_glyphs[run.start..run.end];
            let Some(input) = self.prepare_run_inputs(span, color, opacity, &run, glyphs)? else {
                return Ok(false);
            };
            let mut prepared_span = self.prepared_run(glyphs, input, policy);
            prepared_span.effects = effects.take();
            prepared.push(prepared_span);
        }
        Ok(true)
    }

    fn prepare_run_inputs(
        &self,
        span: &usvg::layout::Span,
        color: usvg::Color,
        opacity: u8,
        run: &runs::GlyphRun,
        glyphs: &[usvg::layout::PositionedGlyph],
    ) -> Result<Option<SpanInput>, String> {
        let Some((face, variable_instance)) =
            self.ensure_run_face(span, run.font, run.font_size)?
        else {
            return Ok(None);
        };
        /* WHY: usvgの字形サイズはNonZeroPositiveF32の元spanからそのまま渡る。 */
        let Some(glyph_ids) = glyph_ids(glyphs) else {
            return Ok(None);
        };
        Ok(Some(SpanInput {
            color,
            opacity,
            font: run.font,
            face,
            variable_instance,
            font_size: run.font_size,
            glyph_ids,
        }))
    }

    fn prepared_run(
        &self,
        glyphs: &[usvg::layout::PositionedGlyph],
        input: SpanInput,
        policy: Option<baseline::Policy>,
    ) -> PreparedSpan {
        let baseline_shift = policy
            .map(|value| baseline::glyph_shift(self.database, input.font, &input.face, value))
            .unwrap_or(0.0);
        PreparedSpan {
            fill: [
                input.color.red,
                input.color.green,
                input.color.blue,
                input.opacity,
            ],
            font: input.font,
            glyphs: glyphs.to_vec(),
            glyph_ids: input.glyph_ids,
            upem: input.face.upem,
            typeface: input.face.typeface,
            font_size: input.font_size,
            baseline_shift,
            x_alignment: None,
            chunk_alignment: None,
            fallback_alignment: None,
            effects: None,
            variable_instance: input.variable_instance,
        }
    }
}

fn effects_runs_supported(span: &usvg::layout::Span, has_effects: bool) -> bool {
    !has_effects || runs::consecutive_runs(&span.positioned_glyphs).count() == 1
}

fn prepare_effects(
    span: &usvg::layout::Span,
) -> Result<Option<Option<Box<decorations::Effects>>>, String> {
    if !decorations::has_effects(span) {
        return Ok(Some(None));
    }
    Ok(decorations::prepare(span)?.map(|effects| Some(Box::new(effects))))
}

fn glyph_ids(glyphs: &[usvg::layout::PositionedGlyph]) -> Option<Vec<u16>> {
    glyphs
        .iter()
        .map(|glyph| u16::try_from(glyph.id.0).ok())
        .collect()
}

fn span_supported(span: &usvg::layout::Span) -> bool {
    span.visible
        && (decorations::has_effects(span) || span.paint_order == usvg::PaintOrder::FillAndStroke)
}

fn solid_fill(span: &usvg::layout::Span) -> Option<(usvg::Color, u8)> {
    let fill = span.fill.as_ref()?;
    let usvg::Paint::Color(color) = fill.paint() else {
        return None;
    };
    Some((*color, fill.opacity().to_u8()))
}
