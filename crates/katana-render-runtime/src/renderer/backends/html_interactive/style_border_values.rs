use super::super::types::CssBorderStyle;

const BORDER_SIDE_COUNT: usize = 4;

pub(super) fn border_style(value: &str) -> Option<CssBorderStyle> {
    value
        .split_whitespace()
        .find_map(|token| match token.to_ascii_lowercase().as_str() {
            "solid" => Some(CssBorderStyle::Solid),
            "dashed" => Some(CssBorderStyle::Dashed),
            "dotted" => Some(CssBorderStyle::Dotted),
            "none" | "hidden" => Some(CssBorderStyle::None),
            _ => None,
        })
}

pub(super) fn border_styles(value: &str) -> Option<[CssBorderStyle; BORDER_SIDE_COUNT]> {
    let values = value
        .split_whitespace()
        .filter_map(border_style)
        .collect::<Vec<_>>();
    match values.as_slice() {
        [one] => Some([*one; BORDER_SIDE_COUNT]),
        [top, right] => Some([*top, *right, *top, *right]),
        [top, right, bottom] => Some([*top, *right, *bottom, *right]),
        [top, right, bottom, left] => Some([*top, *right, *bottom, *left]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{border_style, border_styles};
    use crate::renderer::backends::html_interactive::style::CssBorderStyle;

    #[test]
    fn border_style_accepts_supported_keywords_and_ignores_unknown_tokens() {
        assert_eq!(border_style("2px solid red"), Some(CssBorderStyle::Solid));
        assert_eq!(border_style("dashed"), Some(CssBorderStyle::Dashed));
        assert_eq!(border_style("dotted"), Some(CssBorderStyle::Dotted));
        assert_eq!(border_style("hidden"), Some(CssBorderStyle::None));
        assert_eq!(border_style("2px invalid"), None);
    }

    #[test]
    fn border_styles_expands_one_to_three_values() {
        assert_eq!(border_styles("solid"), Some([CssBorderStyle::Solid; 4]));
        assert_eq!(
            border_styles("solid dashed"),
            Some([
                CssBorderStyle::Solid,
                CssBorderStyle::Dashed,
                CssBorderStyle::Solid,
                CssBorderStyle::Dashed,
            ])
        );
        assert_eq!(
            border_styles("solid dashed dotted"),
            Some([
                CssBorderStyle::Solid,
                CssBorderStyle::Dashed,
                CssBorderStyle::Dotted,
                CssBorderStyle::Dashed,
            ])
        );
    }

    #[test]
    fn border_styles_accepts_four_sides_and_rejects_excess_values() {
        assert_eq!(
            border_styles("solid dashed dotted none"),
            Some([
                CssBorderStyle::Solid,
                CssBorderStyle::Dashed,
                CssBorderStyle::Dotted,
                CssBorderStyle::None,
            ])
        );
        assert_eq!(border_styles("solid dashed dotted none hidden"), None);
    }
}
