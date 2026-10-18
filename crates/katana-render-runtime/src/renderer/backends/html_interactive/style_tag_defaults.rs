use super::super::constants::{
    FONT_WEIGHT_BOLD, H1_FONT_SCALE, H1_MARGIN_SCALE, H2_FONT_SCALE, H2_MARGIN_SCALE,
    H3_FONT_SCALE, H3_MARGIN_SCALE, H4_FONT_SCALE, H4_MARGIN_SCALE, H5_FONT_SCALE, H5_MARGIN_SCALE,
    H6_FONT_SCALE, H6_MARGIN_SCALE,
};
use super::CssStyle;

impl CssStyle {
    pub(super) fn apply_tag_metrics(&mut self, tag: &str) {
        if is_phrasing_tag(tag) {
            self.inline_block = true;
            self.inline_atomic = false;
        }
        match tag {
            "h1" => self.apply_heading(H1_FONT_SCALE, H1_MARGIN_SCALE),
            "h2" => self.apply_heading(H2_FONT_SCALE, H2_MARGIN_SCALE),
            "h3" => self.apply_heading(H3_FONT_SCALE, H3_MARGIN_SCALE),
            "h4" => self.apply_heading(H4_FONT_SCALE, H4_MARGIN_SCALE),
            "h5" => self.apply_heading(H5_FONT_SCALE, H5_MARGIN_SCALE),
            "h6" => self.apply_heading(H6_FONT_SCALE, H6_MARGIN_SCALE),
            "p" => {
                self.margin_top = self.font_size;
                self.margin_bottom = self.font_size;
            }
            "b" | "strong" | "th" => self.font_weight = FONT_WEIGHT_BOLD,
            "thead" | "tbody" | "tfoot" => {
                self.vertical_align = super::CssVerticalAlign::Middle;
            }
            "em" | "i" => self.italic = true,
            "code" => self.font_family = "monospace".to_string(),
            _ => {}
        }
    }

    pub(super) fn refresh_tag_margins(&mut self, tag: &str) {
        match tag {
            "h1" => self.set_heading_margin(H1_MARGIN_SCALE),
            "h2" => self.set_heading_margin(H2_MARGIN_SCALE),
            "h3" => self.set_heading_margin(H3_MARGIN_SCALE),
            "h4" => self.set_heading_margin(H4_MARGIN_SCALE),
            "h5" => self.set_heading_margin(H5_MARGIN_SCALE),
            "h6" => self.set_heading_margin(H6_MARGIN_SCALE),
            "p" => {
                self.margin_top = self.font_size;
                self.margin_bottom = self.font_size;
            }
            _ => {}
        }
    }

    fn apply_heading(&mut self, font_scale: f32, margin_scale: f32) {
        self.font_size *= font_scale;
        self.set_heading_margin(margin_scale);
        self.font_weight = FONT_WEIGHT_BOLD;
    }

    fn set_heading_margin(&mut self, margin_scale: f32) {
        let margin = self.font_size * margin_scale;
        self.margin_top = margin;
        self.margin_bottom = margin;
    }
}

fn is_phrasing_tag(tag: &str) -> bool {
    is_phrasing_text_tag(tag) || is_inline_control_tag(tag)
}

fn is_phrasing_text_tag(tag: &str) -> bool {
    matches!(
        tag,
        "a" | "abbr"
            | "b"
            | "bdi"
            | "bdo"
            | "cite"
            | "code"
            | "dfn"
            | "em"
            | "i"
            | "kbd"
            | "mark"
            | "q"
            | "s"
            | "samp"
            | "small"
            | "span"
            | "strong"
            | "sub"
            | "sup"
            | "time"
            | "u"
            | "var"
    )
}

fn is_inline_control_tag(tag: &str) -> bool {
    matches!(tag, "button" | "input" | "select" | "textarea")
}
