use super::super::constants::{
    INPUT_HEIGHT, MONOSPACE_CHARACTER_WIDTH_FACTOR, TEXT_CHARACTER_WIDTH_FACTOR,
};
use super::{CssPosition, CssStyle};

const CSS_LAYOUT_SUBPIXEL_SCALE: f32 = 64.0;

impl CssStyle {
    pub(in super::super) fn differs_in_inline_metrics(&self, other: &Self) -> bool {
        self.font_size != other.font_size
            || self.line_height != other.line_height
            || self.line_height_factor != other.line_height_factor
            || self.font_family != other.font_family
            || self.font_weight != other.font_weight
            || self.italic != other.italic
            || self.letter_spacing != other.letter_spacing
    }

    pub(in super::super) fn is_plain_inherited_inline(&self, inherited: &Self) -> bool {
        !self.inline_atomic && plain_inline_box(self) && !self.differs_in_inline_metrics(inherited)
    }

    pub(in super::super) fn has_inline_metric_change(
        &self,
        children: &[super::super::super::html_document::HtmlDocumentNode],
    ) -> bool {
        children.iter().any(|child| {
            let super::super::super::html_document::HtmlDocumentNode::Element {
                tag,
                attributes,
                children,
                ..
            } = child
            else {
                return false;
            };
            let style = Self::from_element(tag, attributes, self);
            style.differs_in_inline_metrics(self) || style.has_inline_metric_change(children)
        })
    }

    pub(in crate::renderer::backends::html_interactive) fn text_character_width_factor(
        &self,
    ) -> f32 {
        if self.font_family.to_ascii_lowercase().contains("mono") {
            MONOSPACE_CHARACTER_WIDTH_FACTOR
        } else {
            TEXT_CHARACTER_WIDTH_FACTOR
        }
    }

    pub(in super::super) fn resolve_text_line_height(
        &mut self,
        children: &[super::super::super::html_document::HtmlDocumentNode],
    ) {
        if !self.normal_line_height {
            self.line_height = self.used_explicit_line_height();
            return;
        }
        let text = super::super::document::node_text(children);
        let measured = crate::markdown::svg_rasterize::SvgRasterizeOps::html_line_metrics(
            &text,
            &self.font_family,
            self.font_size,
            self.font_weight,
            self.italic,
        )
        .map_or(self.line_height, |(ascent, descent)| {
            (ascent + descent).round()
        });
        self.line_height = Self::inline_line_box_height(children, self).unwrap_or(measured);
    }

    fn inline_line_box_height(
        children: &[super::super::super::html_document::HtmlDocumentNode],
        inherited: &Self,
    ) -> Option<f32> {
        let mut line_height = inherited.line_height;
        let mut has_inline_item = false;
        for child in children {
            if let Some(item_height) = Self::inline_item_height(child, inherited) {
                has_inline_item = true;
                line_height = line_height.max(item_height);
            }
        }
        has_inline_item.then_some(line_height)
    }

    fn inline_item_height(
        child: &super::super::super::html_document::HtmlDocumentNode,
        inherited: &Self,
    ) -> Option<f32> {
        let super::super::super::html_document::HtmlDocumentNode::Element {
            tag,
            attributes,
            children,
            ..
        } = child
        else {
            return None;
        };
        let control = Self::from_element(tag, attributes, inherited);
        if matches!(control.position, CssPosition::Absolute | CssPosition::Fixed)
            || !control.inline_block
            || control.display == taffy::style::Display::None
        {
            return None;
        }
        if tag != "input"
            && Self::inherits_plain_inline_metrics(&control, inherited)
            && !control.has_inline_metric_change(children)
        {
            return None;
        }
        Self::inline_element_height(tag, children, control)
    }

    fn inherits_plain_inline_metrics(control: &Self, inherited: &Self) -> bool {
        control.is_plain_inherited_inline(inherited)
    }

    fn inline_element_height(
        tag: &str,
        children: &[super::super::super::html_document::HtmlDocumentNode],
        control: Self,
    ) -> Option<f32> {
        if tag == "input" {
            return Some(Self::input_outer_height(&control));
        }
        let measured = crate::markdown::svg_rasterize::SvgRasterizeOps::html_line_metrics(
            &super::super::document::node_text(children),
            &control.font_family,
            control.font_size,
            control.font_weight,
            control.italic,
        )
        .map(|(ascent, descent)| (ascent + descent).round());
        Self::inline_line_box_height(children, &control)
            .map(|nested| nested.max(measured.unwrap_or(0.0)))
            .or(measured)
    }

    fn input_outer_height(style: &Self) -> f32 {
        style
            .height
            .map(|height| style.outer_height(height))
            .unwrap_or(INPUT_HEIGHT)
            .max(style.minimum_outer_height())
    }
    fn used_explicit_line_height(&self) -> f32 {
        match self.line_height_factor {
            Some(factor) => {
                /* WHY: 単位なし行高は使用font-sizeを量子化してから乗算する。px値とは丸め順が異なる。 */
                let used_size = (self.font_size * CSS_LAYOUT_SUBPIXEL_SCALE).round()
                    / CSS_LAYOUT_SUBPIXEL_SCALE;
                (used_size * factor * CSS_LAYOUT_SUBPIXEL_SCALE).floor() / CSS_LAYOUT_SUBPIXEL_SCALE
            }
            None => {
                (self.line_height * CSS_LAYOUT_SUBPIXEL_SCALE).round() / CSS_LAYOUT_SUBPIXEL_SCALE
            }
        }
    }
}

fn plain_inline_box(style: &CssStyle) -> bool {
    style.background.is_none()
        && style.box_shadow.is_none()
        && style.border_width == 0.0
        && style.border_top_width() == 0.0
        && style.border_right_width() == 0.0
        && style.border_bottom_width() == 0.0
        && style.border_left_width() == 0.0
        && style.padding_top == 0.0
        && style.padding_right == 0.0
        && style.padding_bottom == 0.0
        && style.padding_left == 0.0
}

#[cfg(test)]
mod tests {
    use super::CssStyle;

    #[test]
    fn normal_body_line_height_keeps_the_inherited_inline_strut() {
        use crate::renderer::backends::html_document::HtmlDocumentNode;

        let mut style = CssStyle::browser_default();
        style.resolve_text_line_height(&[
            HtmlDocumentNode::Text("名称".to_string()),
            HtmlDocumentNode::Element {
                node_id: 1,
                tag: "input".to_string(),
                attributes: Vec::new(),
                children: Vec::new(),
            },
        ]);

        assert_eq!(style.line_height, 22.0);
    }

    #[test]
    fn inline_line_box_uses_the_tallest_control_and_text_item() {
        let mut style = CssStyle::browser_default();
        let children = tall_control_children();
        assert_eq!(
            CssStyle::inline_item_height(&children[0], &style),
            Some(40.0)
        );
        assert_eq!(
            CssStyle::inline_line_box_height(&children, &style),
            Some(40.0)
        );
        style.resolve_text_line_height(&children);

        assert_eq!(style.line_height, 40.0);
    }

    #[test]
    fn only_out_of_flow_controls_leave_the_inline_line_box() {
        use crate::renderer::backends::html_document::HtmlDocumentNode;

        let style = CssStyle::browser_default();
        let relative = HtmlDocumentNode::Element {
            node_id: 3,
            tag: "input".to_string(),
            attributes: vec![(
                "style".to_string(),
                "height:40px;position:relative".to_string(),
            )],
            children: Vec::new(),
        };
        let absolute = HtmlDocumentNode::Element {
            node_id: 4,
            tag: "input".to_string(),
            attributes: vec![(
                "style".to_string(),
                "height:40px;position:absolute".to_string(),
            )],
            children: Vec::new(),
        };

        assert_eq!(CssStyle::inline_item_height(&relative, &style), Some(40.0));
        assert_eq!(CssStyle::inline_item_height(&absolute, &style), None);
    }

    fn tall_control_children() -> [crate::renderer::backends::html_document::HtmlDocumentNode; 2] {
        use crate::renderer::backends::html_document::HtmlDocumentNode;

        [
            HtmlDocumentNode::Element {
                node_id: 1,
                tag: "input".to_string(),
                attributes: vec![("style".to_string(), "height:40px".to_string())],
                children: Vec::new(),
            },
            HtmlDocumentNode::Element {
                node_id: 2,
                tag: "input".to_string(),
                attributes: vec![("style".to_string(), "height:30px".to_string())],
                children: Vec::new(),
            },
        ]
    }

    #[test]
    fn explicit_line_height_matches_chrome_used_layout_precision() {
        for (specified, expected) in [
            (29.6, 29.59375),
            (20.8, 20.796875),
            (40.0, 40.0),
            (26.656, 26.65625),
            (34.51, 34.515625),
            (20.01, 20.015625),
            (20.005, 20.0),
            (20.025, 20.03125),
        ] {
            let mut style = CssStyle::browser_default();
            style.normal_line_height = false;
            style.line_height_factor = None;
            style.line_height = specified;
            style.resolve_text_line_height(&[]);
            assert_eq!(style.line_height, expected);
            assert_eq!(style.line_height * 3.0, expected * 3.0);
        }
    }

    #[test]
    fn unitless_line_height_uses_quantized_font_size_before_multiplication() {
        for (size, factor, expected) in [
            (11.62, 1.7, 19.75),
            (11.76, 1.7, 20.0),
            (20.3, 1.7, 34.5),
            (12.025, 1.7, 20.453125),
            (13.1, 1.65, 21.59375),
            (17.37, 1.7, 29.53125),
        ] {
            let mut style = CssStyle::browser_default();
            style.font_size = size;
            style.line_height = size * factor;
            style.line_height_factor = Some(factor);
            style.normal_line_height = false;
            style.resolve_text_line_height(&[]);
            assert_eq!(style.line_height, expected);
        }
    }
}
