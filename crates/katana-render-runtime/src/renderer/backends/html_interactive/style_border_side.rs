use super::super::CssStyle;
use super::super::types::CssBorderStyle;

impl CssStyle {
    pub(in crate::renderer::backends::html_interactive) fn border_side_width(
        &self,
        side: &str,
    ) -> f32 {
        match side {
            "top" => self.border_top_width,
            "right" => self.border_right_width,
            "bottom" => self.border_bottom_width,
            "left" => self.border_left_width,
            _ => None,
        }
        .unwrap_or(self.border_width)
    }

    pub(in crate::renderer::backends::html_interactive) fn set_border_side_width(
        &mut self,
        side: &str,
        width: f32,
    ) {
        match side {
            "top" => self.border_top_width = Some(width),
            "right" => self.border_right_width = Some(width),
            "bottom" => self.border_bottom_width = Some(width),
            "left" => self.border_left_width = Some(width),
            _ => {}
        }
    }

    pub(in crate::renderer::backends::html_interactive) fn set_border_side_style(
        &mut self,
        side: &str,
        style: CssBorderStyle,
    ) {
        match side {
            "top" => self.border_top_style = Some(style),
            "right" => self.border_right_style = Some(style),
            "bottom" => self.border_bottom_style = Some(style),
            "left" => self.border_left_style = Some(style),
            _ => {}
        }
    }

    pub(in crate::renderer::backends::html_interactive) fn set_border_side_color(
        &mut self,
        side: &str,
        color: String,
    ) {
        match side {
            "top" => self.border_top_color = Some(color),
            "right" => self.border_right_color = Some(color),
            "bottom" => self.border_bottom_color = Some(color),
            "left" => self.border_left_color = Some(color),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::CssStyle;
    use crate::renderer::backends::html_interactive::style::CssBorderStyle;

    #[test]
    fn unknown_border_side_is_ignored_by_width_style_and_color_setters() {
        let mut style = CssStyle::browser_default();
        style.set_border_side_width("diagonal", 4.0);
        style.set_border_side_style("diagonal", CssBorderStyle::Dashed);
        style.set_border_side_color("diagonal", "red".to_string());

        assert!(style.border_top_width.is_none());
        assert!(style.border_right_width.is_none());
        assert!(style.border_bottom_width.is_none());
        assert!(style.border_left_width.is_none());
        assert!(style.border_top_style.is_none());
        assert!(style.border.is_none());
    }
}
