use super::super::super::document::{border_color, css_px};
use super::super::types::CssBorderStyle;
use super::CssStyle;
use super::border_values::{border_style, border_styles};

impl CssStyle {
    pub(in crate::renderer::backends::html_interactive) fn apply_border(&mut self, value: &str) {
        self.explicit_border = true;
        self.clear_border_edge_overrides();
        if value.trim().eq_ignore_ascii_case("none") {
            self.border = None;
            self.border_style = CssBorderStyle::None;
            self.border_width = 0.0;
            return;
        }
        self.border_style = border_style(value).unwrap_or(CssBorderStyle::None);
        self.clear_border_edge_styles();
        self.border = border_color(value).or_else(|| self.border.clone());
        self.border_width = value
            .split_whitespace()
            .find_map(css_px)
            .unwrap_or_else(|| {
                if self.border.is_some() {
                    1.0
                } else {
                    self.border_width
                }
            });
    }

    pub(in crate::renderer::backends::html_interactive) fn apply_border_style(
        &mut self,
        value: &str,
    ) {
        self.explicit_border = true;
        let Some([top, right, bottom, left]) = border_styles(value) else {
            return;
        };
        self.border_style = top;
        self.border_top_style = Some(top);
        self.border_right_style = (right != top).then_some(right);
        self.border_bottom_style = (bottom != top).then_some(bottom);
        self.border_left_style = (left != top).then_some(left);
    }

    pub(in crate::renderer::backends::html_interactive) fn apply_border_color(
        &mut self,
        value: &str,
    ) {
        self.explicit_border = true;
        let Some(color) = border_color(value) else {
            return;
        };
        self.border = Some(color);
        self.border_top_color = None;
        self.border_right_color = None;
        self.border_bottom_color = None;
        self.border_left_color = None;
    }

    pub(in crate::renderer::backends::html_interactive) fn apply_border_side(
        &mut self,
        name: &str,
        value: &str,
    ) {
        self.explicit_border = true;
        let side = name
            .to_ascii_lowercase()
            .strip_prefix("border-")
            .map(str::to_string);
        let Some(side) = side else {
            return;
        };
        let color = border_color(value);
        let style = border_style(value);
        let current_width = self.border_side_width(&side);
        let width = if value.trim().eq_ignore_ascii_case("none") {
            current_width
        } else {
            value
                .split_whitespace()
                .find_map(css_px)
                .unwrap_or_else(|| if color.is_some() { 1.0 } else { current_width })
        };
        self.set_border_side_width(&side, width);
        self.set_border_side_style(&side, style.unwrap_or(CssBorderStyle::None));
        if let Some(color) = color {
            self.set_border_side_color(&side, color);
        }
    }

    pub(in crate::renderer::backends::html_interactive) fn apply_border_side_style(
        &mut self,
        name: &str,
        value: &str,
    ) {
        self.explicit_border = true;
        let Some(style) = border_style(value) else {
            return;
        };
        let Some(side) = name
            .to_ascii_lowercase()
            .strip_prefix("border-")
            .and_then(|value| value.strip_suffix("-style"))
            .map(str::to_string)
        else {
            return;
        };
        self.set_border_side_style(&side, style);
    }

    pub(in crate::renderer::backends::html_interactive) fn apply_border_side_color(
        &mut self,
        name: &str,
        value: &str,
    ) {
        self.explicit_border = true;
        let Some(color) = border_color(value) else {
            return;
        };
        let lower_name = name.to_ascii_lowercase();
        let side = lower_name
            .strip_prefix("border-")
            .and_then(|value| value.strip_suffix("-color"));
        if let Some(side) = side {
            self.set_border_side_color(side, color);
        }
    }

    fn clear_border_edge_overrides(&mut self) {
        self.border_top_width = None;
        self.border_right_width = None;
        self.border_bottom_width = None;
        self.border_left_width = None;
        self.border_top_color = None;
        self.border_right_color = None;
        self.border_bottom_color = None;
        self.border_left_color = None;
        self.clear_border_edge_styles();
    }

    fn clear_border_edge_styles(&mut self) {
        self.border_top_style = None;
        self.border_right_style = None;
        self.border_bottom_style = None;
        self.border_left_style = None;
    }
}

#[cfg(test)]
mod tests {
    use super::super::CssStyle;
    use crate::renderer::backends::html_interactive::style::CssBorderStyle;

    #[test]
    fn invalid_border_shorthands_preserve_existing_state() {
        let mut style = CssStyle::browser_default();
        style.apply_border_style("solid dashed dotted none");
        style.border_width = 3.0;

        for invalid in ["unsupported", "solid dashed dotted none hidden"] {
            style.apply_border_style(invalid);
            assert_eq!(style.border_style, CssBorderStyle::Solid);
            assert_eq!(style.border_width, 3.0);
            assert_eq!(style.border_right_style, Some(CssBorderStyle::Dashed));
            assert_eq!(style.border_bottom_style, Some(CssBorderStyle::Dotted));
            assert_eq!(style.border_left_style, Some(CssBorderStyle::None));
        }
    }

    #[test]
    fn invalid_border_side_names_do_not_change_edges() {
        let mut style = CssStyle::browser_default();
        style.apply_border_side("border-diagonal", "4px solid red");
        style.apply_border_side_style("border-diagonal-style", "dashed");
        style.apply_border_side_color("border-diagonal-color", "red");

        assert!(style.border_top_width.is_none());
        assert!(style.border_right_width.is_none());
        assert!(style.border_bottom_width.is_none());
        assert!(style.border_left_width.is_none());
    }

    #[test]
    fn invalid_border_side_style_keeps_prior_edge_and_records_authored_border() {
        let mut style = CssStyle::browser_default();
        style.apply_border_side_style("border-top-style", "solid");
        style.apply_border_side_style("border-top-style", "unsupported");
        style.apply_border_side_style("border-top", "dashed");
        assert_eq!(style.border_top_style, Some(CssBorderStyle::Solid));
        assert!(style.explicit_border);
    }
}
