use super::layout::HtmlLayoutRenderer;
#[path = "layout_paint_text.rs"]
pub(super) mod paint_text;
use super::style::CssStyle;
use super::svg::box_svg;

impl HtmlLayoutRenderer {
    pub(super) fn paint_box(&mut self, x: f32, y: f32, width: f32, height: f32, style: &CssStyle) {
        let gradient_id = self.next_gradient_id;
        self.next_gradient_id += 1;
        self.svg.push_str(&box_svg(
            gradient_id,
            x - self.scroll_x,
            y - self.scroll_y,
            width,
            height,
            style,
        ));
    }

    pub(super) fn insert_box(
        &mut self,
        index: usize,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        style: &CssStyle,
    ) {
        let gradient_id = self.next_gradient_id;
        self.next_gradient_id += 1;
        self.svg.insert_str(
            index,
            &box_svg(
                gradient_id,
                x - self.scroll_x,
                y - self.scroll_y,
                width,
                height,
                style,
            ),
        );
    }
}
