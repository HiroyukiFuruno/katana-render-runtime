use super::super::constants::{DEFAULT_FONT_SIZE, DEFAULT_LINE_HEIGHT, FONT_WEIGHT_NORMAL};
use super::super::document::attribute;
use super::{CssBoxSizing, CssGridTrack, CssStyle};
use taffy::style::{Display, FlexDirection, FlexWrap};

#[cfg(test)]
const DEFAULT_STYLE_VIEWPORT_WIDTH: f32 = 1_024.0;
#[cfg(test)]
const DEFAULT_STYLE_VIEWPORT_HEIGHT: f32 = 768.0;

impl CssStyle {
    #[cfg(test)]
    pub(in super::super) fn browser_default() -> Self {
        Self::browser_default_for_viewport(
            DEFAULT_STYLE_VIEWPORT_WIDTH,
            DEFAULT_STYLE_VIEWPORT_HEIGHT,
        )
    }

    pub(in super::super) fn browser_default_for_viewport(
        viewport_width: f32,
        viewport_height: f32,
    ) -> Self {
        Self::default_with_text_defaults(
            viewport_width,
            viewport_height,
            "#000000".to_string(),
            "serif".to_string(),
        )
    }

    fn default_with_text_defaults(
        viewport_width: f32,
        viewport_height: f32,
        color: String,
        font_family: String,
    ) -> Self {
        let mut style = Self {
            color,
            opacity: 1.0,
            font_size: DEFAULT_FONT_SIZE,
            line_height: DEFAULT_LINE_HEIGHT,
            line_height_factor: Some(DEFAULT_LINE_HEIGHT / DEFAULT_FONT_SIZE),
            normal_line_height: true,
            font_family,
            font_weight: FONT_WEIGHT_NORMAL,
            display: Display::Block,
            inline_block: false,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            flex_shrink: 1.0,
            automatic_min_height: true,
            grid_template_columns: vec![CssGridTrack::Fraction(1.0)],
            viewport_width,
            viewport_height,
            ..<Self as Default>::default()
        };
        style.column_gap = style.font_size;
        style
    }

    #[cfg(test)]
    pub(in super::super) fn from_attributes(
        attributes: &[(String, String)],
        inherited: &Self,
    ) -> Self {
        Self::from_element_attributes(None, attributes, inherited)
    }

    pub(in super::super) fn from_element(
        tag: &str,
        attributes: &[(String, String)],
        inherited: &Self,
    ) -> Self {
        Self::from_element_attributes(Some(tag), attributes, inherited)
    }

    fn from_element_attributes(
        tag: Option<&str>,
        attributes: &[(String, String)],
        inherited: &Self,
    ) -> Self {
        let started = super::super::text_measurement_diagnostic::start_style_timing();
        let style = Self::from_element_attributes_body(tag, attributes, inherited);
        super::super::text_measurement_diagnostic::record_style_timing(started);
        style
    }

    fn from_element_attributes_body(
        tag: Option<&str>,
        attributes: &[(String, String)],
        inherited: &Self,
    ) -> Self {
        let mut style = inherited.element_defaults();
        if let Some(tag) = tag {
            style.apply_tag_metrics(tag);
            if tag == "tr" || tag == "td" || tag == "th" {
                style.vertical_align = inherited.vertical_align;
            }
            /* WHY: Chromium のフォーム部品は既定で border-box として寸法を解決する。 */
            if tag == "input" || tag == "textarea" {
                style.box_sizing = CssBoxSizing::BorderBox;
            }
        }
        if attribute(attributes, "hidden").is_some() {
            style.display = Display::None;
        }
        let Some(source) = attribute(attributes, "style") else {
            return style;
        };
        style.apply_element_declarations(
            source,
            inherited.font_size,
            inherited.vertical_align,
            tag,
        );
        style
    }

    fn element_defaults(&self) -> Self {
        Self {
            font_size: self.font_size,
            line_height: self.line_height,
            line_height_factor: self.line_height_factor,
            normal_line_height: self.normal_line_height,
            explicit_typography: false,
            font_feature_settings: self.font_feature_settings.clone(),
            text_spacing_trim: self.text_spacing_trim,
            font_weight: self.font_weight,
            column_gap: self.font_size,
            italic: self.italic,
            underline: self.underline,
            text_align: self.text_align,
            text_transform: self.text_transform,
            white_space: self.white_space,
            list_style_none: self.list_style_none,
            letter_spacing: self.letter_spacing,
            viewport_width: self.viewport_width,
            viewport_height: self.viewport_height,
            percentage_height_basis: self.percentage_height_basis,
            ..Self::default_with_text_defaults(
                self.viewport_width,
                self.viewport_height,
                self.color.clone(),
                self.font_family.clone(),
            )
        }
    }

    pub(in super::super) fn inherited_text_style(&self) -> Self {
        self.element_defaults()
    }

    pub(in super::super) fn resolve_inherited_line_height(&mut self) {
        if let Some(factor) = self.line_height_factor {
            self.line_height = self.font_size * factor;
        }
    }
}
