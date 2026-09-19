use super::super::constants::MIN_LAYOUT_WIDTH;
use super::super::layout::HtmlLayoutRenderer;
use super::super::style::{CssStyle, CssTextAlign};
use super::InlineMeasurement;
use super::state::InlineFlowState;

struct InlinePaintContext<'a> {
    initial_x: f32,
    initial_y: f32,
    style: &'a CssStyle,
    inline: &'a InlineFlowState,
    paint_style: &'a CssStyle,
}

impl HtmlLayoutRenderer {
    pub(super) fn paint_inline_lines(
        &mut self,
        lines: &[String],
        initial_x: f32,
        initial_y: f32,
        style: &CssStyle,
        inline: &InlineFlowState,
    ) {
        let mut paint_style = style.clone();
        paint_style.text_align = CssTextAlign::Start;
        let context = InlinePaintContext {
            initial_x,
            initial_y,
            style,
            inline,
            paint_style: &paint_style,
        };
        for (index, line) in lines.iter().enumerate() {
            self.paint_inline_line(line, index, &context);
        }
    }

    fn paint_inline_line(&mut self, line: &str, index: usize, context: &InlinePaintContext<'_>) {
        let line_x = if index == 0 {
            context.initial_x
        } else {
            context.inline.x
        };
        let (leading_width, visible_line) = InlineMeasurement::visible_line(line, context.style);
        let line_x = line_x + leading_width;
        let line_y = context.initial_y + index as f32 * context.style.line_height;
        let visible_line = visible_line.to_string();
        self.record_inline_fragment(
            line_x,
            line_y,
            InlineMeasurement::text_width(&visible_line, context.style),
            context.style.line_height,
        );
        self.paint_text_lines(
            std::slice::from_ref(&visible_line),
            line_x,
            (context.inline.x + context.inline.width - line_x).max(MIN_LAYOUT_WIDTH),
            line_y + context.style.font_size,
            context.paint_style,
        );
    }
}
