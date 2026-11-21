use super::style_box_declaration_overflow::parse_overflow_declaration;
use crate::renderer::backends::html_interactive::document::css_px;
use crate::renderer::backends::html_interactive::style::value::box_sides;
use crate::renderer::backends::html_interactive::style::{
    CssBoxSizing, CssLength, CssOverflow, CssOverflowAxis, CssStyle, normalized_property_name,
};

impl CssStyle {
    pub(crate) fn apply_box_property(&mut self, name: &str, value: &str) -> bool {
        let name = normalized_property_name(name);
        self.apply_box_property_with_name(name.as_ref(), value)
    }

    fn apply_box_property_with_name(&mut self, name: &str, value: &str) -> bool {
        match name {
            "box-sizing" => return self.apply_simple_box_property("box-sizing", value),
            "overflow" | "overflow-x" | "overflow-y" => {
                return self.apply_overflow_property(name, value);
            }
            _ => {}
        }
        if self.apply_border_box_property(name, value) {
            return true;
        }
        if self.apply_spacing_box_property(name, value) {
            return true;
        }
        if self.apply_dimension_box_property(name, value) {
            return true;
        }
        self.apply_position_box_property(name, value)
    }

    fn apply_simple_box_property(&mut self, name: &str, value: &str) -> bool {
        match name {
            "box-sizing" => {
                self.apply_box_sizing(value);
                true
            }
            "overflow" => {
                self.apply_overflow(value);
                true
            }
            _ => false,
        }
    }

    fn apply_overflow_property(&mut self, name: &str, value: &str) -> bool {
        if !matches!(name, "overflow" | "overflow-x" | "overflow-y") {
            return false;
        }
        let Some(assignment) = parse_overflow_declaration(name, value) else {
            return true;
        };
        if let Some(axis) = assignment.x {
            self.overflow_x = axis;
        }
        if let Some(axis) = assignment.y {
            self.overflow_y = axis;
        }
        self.sync_legacy_overflow();
        true
    }

    fn apply_border_box_property(&mut self, name: &str, value: &str) -> bool {
        match name {
            "border-width" => {
                self.apply_border_widths(value);
                true
            }
            "border-top-width"
            | "border-right-width"
            | "border-bottom-width"
            | "border-left-width" => {
                self.apply_border_side_width(name, value);
                true
            }
            "border-radius" => {
                self.apply_border_radius(value);
                true
            }
            _ => false,
        }
    }

    fn apply_spacing_box_property(&mut self, name: &str, value: &str) -> bool {
        match name {
            "padding" | "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => {
                self.apply_padding_property(name, value);
                true
            }
            "margin" | "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => {
                self.apply_margin_property(name, value);
                true
            }
            _ => false,
        }
    }

    fn apply_dimension_box_property(&mut self, name: &str, value: &str) -> bool {
        match name {
            "width" | "min-width" | "max-width" | "height" | "min-height" | "max-height" => {
                self.apply_dimensions(name, value);
                true
            }
            _ => false,
        }
    }

    fn apply_position_box_property(&mut self, name: &str, value: &str) -> bool {
        if matches!(name, "inset" | "top" | "right" | "bottom" | "left") {
            self.apply_inset(name, value);
            return true;
        }
        false
    }

    fn apply_border_radius(&mut self, value: &str) {
        self.border_radius = CssLength::parse(
            value,
            self.font_size,
            self.viewport_width,
            self.viewport_height,
        )
        .unwrap_or(self.border_radius);
    }

    fn apply_border_widths(&mut self, value: &str) {
        let Some([top, right, bottom, left]) = box_sides(
            value,
            self.font_size,
            self.viewport_width,
            self.viewport_height,
            false,
        ) else {
            return;
        };
        self.border_width = top;
        self.border_top_width = None;
        self.border_right_width = (right != top).then_some(right);
        self.border_bottom_width = (bottom != top).then_some(bottom);
        self.border_left_width = (left != top).then_some(left);
    }

    fn apply_border_side_width(&mut self, name: &str, value: &str) {
        let Some(width) = css_px(value) else {
            return;
        };
        let side = name
            .strip_prefix("border-")
            .and_then(|value| value.strip_suffix("-width"));
        if let Some(side) = side {
            self.set_border_side_width(side, width);
        }
    }

    fn apply_box_sizing(&mut self, value: &str) {
        self.box_sizing = match value.trim().to_ascii_lowercase().as_str() {
            "border-box" => CssBoxSizing::BorderBox,
            "content-box" => CssBoxSizing::ContentBox,
            _ => self.box_sizing,
        };
    }

    fn apply_overflow(&mut self, value: &str) {
        let _ = self.apply_overflow_property("overflow", value);
    }

    fn sync_legacy_overflow(&mut self) {
        self.overflow = CssOverflow::Visible;
        let (x, y) = self.computed_overflow_axes();
        self.overflow = if [x, y]
            .into_iter()
            .any(|axis| axis != CssOverflowAxis::Visible)
        {
            CssOverflow::Clip
        } else {
            CssOverflow::Visible
        };
    }
}

#[cfg(test)]
mod tests {
    use super::{CssBoxSizing, CssOverflow, CssOverflowAxis, CssStyle};

    #[test]
    fn overflow_axis_properties_reuse_the_main_overflow_parser() {
        let mut style = CssStyle::browser_default();

        assert!(style.apply_box_property("overflow-x", "scroll"));
        assert_eq!(style.overflow, CssOverflow::Clip);
        assert_eq!(style.overflow_x, CssOverflowAxis::Scroll);
        assert!(style.apply_box_property("overflow-y", "visible"));
        assert_eq!(style.overflow, CssOverflow::Clip);
        assert_eq!(
            style.computed_overflow_axes(),
            (CssOverflowAxis::Scroll, CssOverflowAxis::Auto)
        );
        assert!(style.apply_box_property("overflow", "unsupported"));
        assert_eq!(style.overflow, CssOverflow::Clip);
        assert_eq!(
            style.specified_overflow_axes(),
            (CssOverflowAxis::Scroll, CssOverflowAxis::Visible)
        );
    }

    #[test]
    fn inset_values_are_accepted_by_position_box_handling() {
        let mut style = CssStyle::browser_default();
        assert!(style.apply_box_property("inset", "1px 2px 3px 4px"));

        assert_eq!(style.inset_top, Some(1.0));
        assert_eq!(style.inset_right, Some(2.0));
        assert_eq!(style.inset_bottom, Some(3.0));
        assert_eq!(style.inset_left, Some(4.0));
    }

    #[test]
    fn box_sizing_stays_same_for_unknown_values() {
        let mut style = CssStyle::browser_default();
        assert!(style.apply_box_property("box-sizing", "unsupported"));
        assert_eq!(style.box_sizing, CssBoxSizing::ContentBox);
    }

    #[test]
    fn simple_box_property_rejects_unknown_names() {
        let mut style = CssStyle::browser_default();
        assert!(!style.apply_simple_box_property("unknown", "value"));
    }

    #[test]
    fn border_side_width_applies_for_valid_side() {
        let mut style = CssStyle::browser_default();
        style.apply_border_side_width("border-top-width", "9px");

        assert_eq!(style.border_top_width, Some(9.0));

        style.apply_border_side_width("width", "12px");
        assert_eq!(style.border_top_width, Some(9.0));
    }

    #[test]
    fn maintained_simple_overflow_helper_preserves_axis_contract_and_rejects_other_names() {
        let mut style = CssStyle::browser_default();
        assert!(style.apply_simple_box_property("overflow", "scroll hidden"));
        assert_eq!(
            style.specified_overflow_axes(),
            (CssOverflowAxis::Scroll, CssOverflowAxis::Hidden)
        );
        assert!(!style.apply_overflow_property("width", "scroll"));
        assert_eq!(
            style.specified_overflow_axes(),
            (CssOverflowAxis::Scroll, CssOverflowAxis::Hidden)
        );
    }
}
