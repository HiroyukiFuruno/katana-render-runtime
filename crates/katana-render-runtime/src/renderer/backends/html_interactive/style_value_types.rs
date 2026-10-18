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
