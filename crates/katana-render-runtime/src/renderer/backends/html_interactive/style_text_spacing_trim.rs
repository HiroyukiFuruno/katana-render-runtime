#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::renderer::backends::html_interactive) enum CssTextSpacingTrim {
    #[default]
    Normal,
    SpaceAll,
}
