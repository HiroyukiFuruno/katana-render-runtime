use super::super::control_style::{input_style, ua_control_style};
use super::super::layout::HtmlLayoutRenderer;
use super::super::style::CssStyle;
use super::super::svg::escape_xml;
use super::super::types::HitTargetKind;
use super::geometry::{
    CHECKBOX_CHECK, CHECKBOX_STROKE, InputGeometry, checkbox_check_ratio_left,
    checkbox_check_ratio_middle_x, checkbox_check_ratio_middle_y, checkbox_check_ratio_right,
    checkbox_check_ratio_top, checkbox_geometry, checkbox_inset, input_geometry,
};

impl HtmlLayoutRenderer {
    pub(crate) fn render_input(
        &mut self,
        node_id: u64,
        attributes: &[(String, String)],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        if super::is_checkbox(attributes) {
            return self.render_checkbox(node_id, attributes, x, y, width, style);
        }
        let control_style = ua_control_style(style);
        let geometry = input_geometry(x, y, width, attributes, &control_style);
        let value = super::values::input_value(self, node_id, attributes);
        let style = input_style(&control_style, self.focused_input == Some(node_id));
        self.paint_input_control(node_id, attributes, &value, geometry, &style);
        geometry.y + geometry.height + style.margin_bottom
    }

    fn render_checkbox(
        &mut self,
        node_id: u64,
        attributes: &[(String, String)],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let geometry = checkbox_geometry(x, y, width, style);
        self.paint_checkbox(geometry, attributes, style);
        self.push_target(
            node_id,
            geometry.x,
            geometry.y,
            geometry.width,
            geometry.height,
            HitTargetKind::Checkbox,
        );
        geometry.y + geometry.height + style.margin_bottom
    }

    fn paint_checkbox(
        &mut self,
        geometry: InputGeometry,
        attributes: &[(String, String)],
        style: &CssStyle,
    ) {
        let inset = checkbox_inset(geometry.width.min(geometry.height));
        let radius = ((geometry.width.min(geometry.height) - inset * 2.0).max(2.0)) / 2.0;
        let center_x = geometry.x + geometry.width / 2.0;
        let center_y = geometry.y + geometry.height / 2.0;
        if style.appearance_none {
            self.paint_box(
                geometry.x,
                geometry.y,
                geometry.width,
                geometry.height,
                style,
            );
        } else {
            self.paint_native_checkbox(center_x, center_y, radius, style);
        }
        if !style.appearance_none && super::values::is_checked(attributes) {
            self.paint_checkbox_checkmark(center_x, center_y, radius);
        }
    }

    fn paint_native_checkbox(
        &mut self,
        center_x: f32,
        center_y: f32,
        radius: f32,
        style: &CssStyle,
    ) {
        self.svg.push_str(&format!(
            r##"<circle cx="{center_x}" cy="{center_y}" r="{radius}" fill="#ffffff" stroke="{}" stroke-width="2"/>"##,
            escape_xml(style.border.as_deref().unwrap_or(CHECKBOX_STROKE))
        ));
    }

    fn paint_checkbox_checkmark(&mut self, center_x: f32, center_y: f32, radius: f32) {
        let left = center_x - radius * checkbox_check_ratio_left();
        let middle_x = center_x - radius * checkbox_check_ratio_middle_x();
        let middle_y = center_y + radius * checkbox_check_ratio_middle_y();
        let right = center_x + radius * checkbox_check_ratio_right();
        let top = center_y - radius * checkbox_check_ratio_top();
        self.svg.push_str(&format!(
            r#"<path d="M {left} {center_y} L {middle_x} {middle_y} L {right} {top}" fill="none" stroke="{CHECKBOX_CHECK}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>"#
        ));
    }

    fn paint_input_control(
        &mut self,
        node_id: u64,
        attributes: &[(String, String)],
        value: &str,
        geometry: InputGeometry,
        style: &CssStyle,
    ) {
        self.paint_box(
            geometry.x,
            geometry.y,
            geometry.width,
            geometry.height,
            style,
        );
        let (value, value_style) = super::values::input_display_value(attributes, value, style);
        self.paint_input_value(value, geometry, &value_style);
        self.push_target(
            node_id,
            geometry.x,
            geometry.y,
            geometry.width,
            geometry.height,
            HitTargetKind::Input,
        );
    }

    fn paint_input_value(&mut self, value: &str, geometry: InputGeometry, style: &CssStyle) {
        let left_inset = if style.padding_left > 0.0 {
            style.border_left_width() + style.padding_left
        } else {
            super::super::constants::INPUT_TEXT_LEFT_PADDING
        };
        let right_inset = if style.padding_right > 0.0 {
            style.border_right_width() + style.padding_right
        } else {
            super::super::constants::INPUT_TEXT_LEFT_PADDING
        };
        self.paint_control_text(
            value,
            geometry.x + left_inset,
            (geometry.width - left_inset - right_inset).max(0.0),
            geometry.y,
            geometry.height,
            style,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{HtmlLayoutRenderer, InputGeometry};
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use crate::renderer::backends::html_interactive::tests::first_text_x;
    use crate::renderer::backends::html_interactive::{
        constants::INPUT_TEXT_LEFT_PADDING,
        style::{CssStyle, CssTextAlign},
        text_metrics::text_width,
    };
    use std::collections::HashMap;

    fn test_geometry() -> InputGeometry {
        InputGeometry {
            x: 0.0,
            y: 0.0,
            width: 160.0,
            height: 24.0,
        }
    }

    fn paint_text_x(style: &CssStyle) -> f32 {
        let viewport = HtmlBrowserViewport {
            width: 300,
            height: 120,
            device_scale_factor: 1.0,
        };
        let geometry = test_geometry();
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        renderer.paint_input_value("AB", geometry, style);
        let painted = first_text_x(&renderer.svg);
        assert!(painted.is_some());
        let mut positions = painted.into_iter().collect::<Vec<_>>();
        positions.remove(0)
    }

    fn expected_end_aligned_x(style: &CssStyle) -> f32 {
        let geometry = test_geometry();
        let left = if style.padding_left > 0.0 {
            style.border_left_width() + style.padding_left
        } else {
            INPUT_TEXT_LEFT_PADDING
        };
        let right = if style.padding_right > 0.0 {
            style.border_right_width() + style.padding_right
        } else {
            INPUT_TEXT_LEFT_PADDING
        };
        let available = (geometry.width - left - right).max(0.0);
        geometry.x + left + (available - text_width("AB", style)).max(0.0)
    }

    #[test]
    fn paint_input_value_uses_default_text_inset_when_paddings_are_empty() {
        let mut style = CssStyle::browser_default();
        style.text_align = CssTextAlign::Start;
        style.padding_left = 0.0;
        style.padding_right = 0.0;

        assert_eq!(paint_text_x(&style), INPUT_TEXT_LEFT_PADDING);
    }

    #[test]
    fn paint_input_value_uses_border_and_padding_inset_when_padding_is_positive() {
        let mut default_style = CssStyle::browser_default();
        default_style.text_align = CssTextAlign::End;
        let default_x = paint_text_x(&default_style);
        let mut padded_style = CssStyle::browser_default();
        padded_style.text_align = CssTextAlign::End;
        padded_style.padding_left = 8.0;
        padded_style.padding_right = 8.0;
        padded_style.border_width = 1.0;
        let padded_x = paint_text_x(&padded_style);

        assert_eq!(default_x, expected_end_aligned_x(&default_style));
        assert_eq!(padded_x, expected_end_aligned_x(&padded_style));
        assert!(
            padded_x >= default_x,
            "default_x={default_x}, padded_x={padded_x}"
        );
    }
}
