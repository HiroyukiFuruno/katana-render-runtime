use taffy::style::{AlignItems, Display, FlexDirection, FlexWrap, JustifyContent};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::renderer::backends::html_interactive) enum CssLength {
    Px(f32),
    Percent(f32),
}

impl Default for CssLength {
    fn default() -> Self {
        Self::Px(0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::renderer::backends::html_interactive) enum CssGridTrack {
    Length(f32),
    Percent(f32),
    Fraction(f32),
    Auto,
    MinContent,
    MaxContent,
    MinMax {
        min: CssGridTrackBreadth,
        max: CssGridTrackBreadth,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::renderer::backends::html_interactive) enum CssGridTrackBreadth {
    Length(f32),
    Percent(f32),
    Fraction(f32),
    Auto,
    MinContent,
    MaxContent,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::renderer::backends::html_interactive) enum CssBoxSizing {
    #[default]
    ContentBox,
    BorderBox,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::renderer::backends::html_interactive) enum CssOverflow {
    #[default]
    Visible,
    Clip,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::renderer::backends::html_interactive) enum CssTextAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::renderer::backends::html_interactive) enum CssTextTransform {
    #[default]
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::renderer::backends::html_interactive) enum CssWhiteSpace {
    #[default]
    Normal,
    NoWrap,
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::renderer::backends::html_interactive) struct CssBoxShadow {
    pub(in crate::renderer::backends::html_interactive) offset_x: f32,
    pub(in crate::renderer::backends::html_interactive) offset_y: f32,
    pub(in crate::renderer::backends::html_interactive) blur_radius: f32,
    pub(in crate::renderer::backends::html_interactive) spread_radius: f32,
    pub(in crate::renderer::backends::html_interactive) color: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::renderer::backends::html_interactive) enum CssPosition {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::renderer::backends::html_interactive) enum CssFloat {
    #[default]
    None,
    Left,
    Right,
}

#[derive(Debug, Clone, Default)]
pub(in crate::renderer::backends::html_interactive) struct CssStyle {
    pub(in crate::renderer::backends::html_interactive) color: String,
    pub(in crate::renderer::backends::html_interactive) background: Option<String>,
    pub(in crate::renderer::backends::html_interactive) box_shadow: Option<CssBoxShadow>,
    pub(in crate::renderer::backends::html_interactive) opacity: f32,
    pub(in crate::renderer::backends::html_interactive) border: Option<String>,
    pub(in crate::renderer::backends::html_interactive) border_width: f32,
    pub(in crate::renderer::backends::html_interactive) border_top_width: Option<f32>,
    pub(in crate::renderer::backends::html_interactive) border_right_width: Option<f32>,
    pub(in crate::renderer::backends::html_interactive) border_bottom_width: Option<f32>,
    pub(in crate::renderer::backends::html_interactive) border_left_width: Option<f32>,
    pub(in crate::renderer::backends::html_interactive) border_top_color: Option<String>,
    pub(in crate::renderer::backends::html_interactive) border_right_color: Option<String>,
    pub(in crate::renderer::backends::html_interactive) border_bottom_color: Option<String>,
    pub(in crate::renderer::backends::html_interactive) border_left_color: Option<String>,
    pub(in crate::renderer::backends::html_interactive) border_radius: CssLength,
    pub(in crate::renderer::backends::html_interactive) box_sizing: CssBoxSizing,
    pub(in crate::renderer::backends::html_interactive) overflow: CssOverflow,
    pub(in crate::renderer::backends::html_interactive) font_size: f32,
    pub(in crate::renderer::backends::html_interactive) line_height: f32,
    pub(in crate::renderer::backends::html_interactive) line_height_factor: Option<f32>,
    pub(in crate::renderer::backends::html_interactive) font_family: String,
    pub(in crate::renderer::backends::html_interactive) font_feature_settings: Option<String>,
    pub(in crate::renderer::backends::html_interactive) italic: bool,
    pub(in crate::renderer::backends::html_interactive) text_align: CssTextAlign,
    pub(in crate::renderer::backends::html_interactive) text_transform: CssTextTransform,
    pub(in crate::renderer::backends::html_interactive) white_space: CssWhiteSpace,
    pub(in crate::renderer::backends::html_interactive) list_style_none: bool,
    pub(in crate::renderer::backends::html_interactive) letter_spacing: f32,
    pub(in crate::renderer::backends::html_interactive) padding_top: f32,
    pub(in crate::renderer::backends::html_interactive) padding_right: f32,
    pub(in crate::renderer::backends::html_interactive) padding_bottom: f32,
    pub(in crate::renderer::backends::html_interactive) padding_left: f32,
    pub(in crate::renderer::backends::html_interactive) margin_top: f32,
    pub(in crate::renderer::backends::html_interactive) margin_right: f32,
    pub(in crate::renderer::backends::html_interactive) margin_bottom: f32,
    pub(in crate::renderer::backends::html_interactive) margin_left: f32,
    pub(in crate::renderer::backends::html_interactive) margin_top_auto: bool,
    pub(in crate::renderer::backends::html_interactive) margin_right_auto: bool,
    pub(in crate::renderer::backends::html_interactive) margin_bottom_auto: bool,
    pub(in crate::renderer::backends::html_interactive) margin_left_auto: bool,
    pub(in crate::renderer::backends::html_interactive) min_height: f32,
    pub(in crate::renderer::backends::html_interactive) automatic_min_height: bool,
    pub(in crate::renderer::backends::html_interactive) width: Option<CssLength>,
    pub(in crate::renderer::backends::html_interactive) min_width: Option<CssLength>,
    pub(in crate::renderer::backends::html_interactive) max_width: Option<CssLength>,
    pub(in crate::renderer::backends::html_interactive) height: Option<f32>,
    pub(in crate::renderer::backends::html_interactive) max_height: Option<f32>,
    pub(in crate::renderer::backends::html_interactive) percentage_height_basis: Option<f32>,
    pub(in crate::renderer::backends::html_interactive) position: CssPosition,
    pub(in crate::renderer::backends::html_interactive) float: CssFloat,
    pub(in crate::renderer::backends::html_interactive) appearance_none: bool,
    pub(in crate::renderer::backends::html_interactive) rotation_degrees: f32,
    pub(in crate::renderer::backends::html_interactive) has_transform: bool,
    pub(in crate::renderer::backends::html_interactive) z_index: Option<i32>,
    pub(in crate::renderer::backends::html_interactive) inset_top: Option<f32>,
    pub(in crate::renderer::backends::html_interactive) inset_right: Option<f32>,
    pub(in crate::renderer::backends::html_interactive) inset_bottom: Option<f32>,
    pub(in crate::renderer::backends::html_interactive) inset_left: Option<f32>,
    pub(in crate::renderer::backends::html_interactive) font_weight: u16,
    pub(in crate::renderer::backends::html_interactive) underline: bool,
    pub(in crate::renderer::backends::html_interactive) explicit_text_decoration: bool,
    pub(in crate::renderer::backends::html_interactive) display: Display,
    pub(in crate::renderer::backends::html_interactive) inline_block: bool,
    pub(in crate::renderer::backends::html_interactive) inline_atomic: bool,
    pub(in crate::renderer::backends::html_interactive) gap: f32,
    pub(in crate::renderer::backends::html_interactive) flex_direction: FlexDirection,
    pub(in crate::renderer::backends::html_interactive) flex_wrap: FlexWrap,
    pub(in crate::renderer::backends::html_interactive) flex_grow: f32,
    pub(in crate::renderer::backends::html_interactive) flex_shrink: f32,
    pub(in crate::renderer::backends::html_interactive) flex_basis: Option<CssLength>,
    pub(in crate::renderer::backends::html_interactive) align_items: Option<AlignItems>,
    pub(in crate::renderer::backends::html_interactive) justify_content: Option<JustifyContent>,
    pub(in crate::renderer::backends::html_interactive) grid_template_columns: Vec<CssGridTrack>,
    pub(in crate::renderer::backends::html_interactive) explicit_color: bool,
    pub(in crate::renderer::backends::html_interactive) explicit_background: bool,
    pub(in crate::renderer::backends::html_interactive) viewport_width: f32,
    pub(in crate::renderer::backends::html_interactive) viewport_height: f32,
}
