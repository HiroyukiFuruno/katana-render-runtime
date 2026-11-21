use super::CssStyle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::renderer::backends::html_interactive) enum CssBorderStyle {
    #[default]
    None,
    Dashed,
    Dotted,
    Solid,
}

impl CssStyle {
    pub(in crate::renderer::backends::html_interactive) fn used_border_width(
        &self,
        side_style: Option<CssBorderStyle>,
        side_width: Option<f32>,
    ) -> f32 {
        if side_style.unwrap_or(self.border_style) == CssBorderStyle::None {
            0.0
        } else {
            side_width.unwrap_or(self.border_width)
        }
    }

    pub(in crate::renderer::backends::html_interactive) fn border_top_dasharray(
        &self,
    ) -> Option<String> {
        Self::dasharray_for_style(
            self.border_top_style.unwrap_or(self.border_style),
            self.border_top_width.unwrap_or(self.border_width),
        )
    }
    pub(in crate::renderer::backends::html_interactive) fn border_right_dasharray(
        &self,
    ) -> Option<String> {
        Self::dasharray_for_style(
            self.border_right_style.unwrap_or(self.border_style),
            self.border_right_width.unwrap_or(self.border_width),
        )
    }
    pub(in crate::renderer::backends::html_interactive) fn border_bottom_dasharray(
        &self,
    ) -> Option<String> {
        Self::dasharray_for_style(
            self.border_bottom_style.unwrap_or(self.border_style),
            self.border_bottom_width.unwrap_or(self.border_width),
        )
    }
    pub(in crate::renderer::backends::html_interactive) fn border_left_dasharray(
        &self,
    ) -> Option<String> {
        Self::dasharray_for_style(
            self.border_left_style.unwrap_or(self.border_style),
            self.border_left_width.unwrap_or(self.border_width),
        )
    }
    pub(in crate::renderer::backends::html_interactive) fn border_top_dotted(&self) -> bool {
        self.border_top_style.unwrap_or(self.border_style) == CssBorderStyle::Dotted
    }
    pub(in crate::renderer::backends::html_interactive) fn border_right_dotted(&self) -> bool {
        self.border_right_style.unwrap_or(self.border_style) == CssBorderStyle::Dotted
    }
    pub(in crate::renderer::backends::html_interactive) fn border_bottom_dotted(&self) -> bool {
        self.border_bottom_style.unwrap_or(self.border_style) == CssBorderStyle::Dotted
    }
    pub(in crate::renderer::backends::html_interactive) fn border_left_dotted(&self) -> bool {
        self.border_left_style.unwrap_or(self.border_style) == CssBorderStyle::Dotted
    }
    pub(in crate::renderer::backends::html_interactive) fn border_dasharray(
        &self,
    ) -> Option<String> {
        Self::dasharray_for_style(self.border_style, self.border_width)
    }
    pub(in crate::renderer::backends::html_interactive) fn border_is_none(&self) -> bool {
        self.border_style == CssBorderStyle::None
    }
    pub(in crate::renderer::backends::html_interactive) fn border_style_is_dotted(&self) -> bool {
        self.border_style == CssBorderStyle::Dotted
    }
    pub(in crate::renderer::backends::html_interactive) fn border_has_dash_style(&self) -> bool {
        self.border_top_dasharray().is_some()
            || self.border_right_dasharray().is_some()
            || self.border_bottom_dasharray().is_some()
            || self.border_left_dasharray().is_some()
    }
    fn dasharray_for_style(style: CssBorderStyle, width: f32) -> Option<String> {
        match style {
            CssBorderStyle::Dashed => Some(format!("{} {}", width * 3.0, width * 3.0)),
            CssBorderStyle::Dotted => Some(format!("{} {}", width, width)),
            CssBorderStyle::Solid | CssBorderStyle::None => None,
        }
    }
}
