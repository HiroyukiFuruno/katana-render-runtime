use crate::renderer::backends::html_interactive::style::CssStyle;
use crate::renderer::backends::html_interactive::style::value::{
    box_sides, css_resolved_px, expand_box_values, split_top_level_whitespace,
};

#[derive(Clone, Copy)]
struct MarginValue {
    length: f32,
    auto: bool,
}

impl CssStyle {
    pub(crate) fn apply_padding_property(&mut self, name: &str, value: &str) {
        self.explicit_padding = true;
        match name {
            "padding" => self.apply_padding(value),
            "padding-top" => {
                self.padding_top = self.box_length(value, false).unwrap_or(self.padding_top);
            }
            "padding-right" => {
                self.padding_right = self.box_length(value, false).unwrap_or(self.padding_right);
            }
            "padding-bottom" => {
                self.padding_bottom = self.box_length(value, false).unwrap_or(self.padding_bottom);
            }
            "padding-left" => {
                self.padding_left = self.box_length(value, false).unwrap_or(self.padding_left);
            }
            _ => {}
        }
    }

    pub(crate) fn apply_margin_property(&mut self, name: &str, value: &str) {
        match name {
            "margin" => self.apply_margin(value),
            "margin-top" => {
                if let Some(value) = self.margin_value(value) {
                    self.margin_top = value.length;
                    self.margin_top_auto = value.auto;
                }
            }
            "margin-right" => {
                if let Some(value) = self.margin_value(value) {
                    self.margin_right = value.length;
                    self.margin_right_auto = value.auto;
                }
            }
            "margin-bottom" => {
                if let Some(value) = self.margin_value(value) {
                    self.margin_bottom = value.length;
                    self.margin_bottom_auto = value.auto;
                }
            }
            "margin-left" => {
                if let Some(value) = self.margin_value(value) {
                    self.margin_left = value.length;
                    self.margin_left_auto = value.auto;
                }
            }
            _ => {}
        }
    }

    fn apply_padding(&mut self, value: &str) {
        let Some([top, right, bottom, left]) = box_sides(
            value,
            self.font_size,
            self.viewport_width,
            self.viewport_height,
            false,
        ) else {
            return;
        };
        self.padding_top = top;
        self.padding_right = right;
        self.padding_bottom = bottom;
        self.padding_left = left;
    }

    fn apply_margin(&mut self, value: &str) {
        let values = split_top_level_whitespace(value)
            .into_iter()
            .map(|value| self.margin_value(value))
            .collect::<Option<Vec<_>>>();
        let Some(values) = values.and_then(expand_margin_values) else {
            return;
        };
        let [top, right, bottom, left] = values;
        self.margin_top = top.length;
        self.margin_right = right.length;
        self.margin_bottom = bottom.length;
        self.margin_left = left.length;
        self.margin_top_auto = top.auto;
        self.margin_right_auto = right.auto;
        self.margin_bottom_auto = bottom.auto;
        self.margin_left_auto = left.auto;
    }

    fn margin_value(&self, value: &str) -> Option<MarginValue> {
        const AUTO_LENGTH: f32 = 0.0;

        if value.trim().eq_ignore_ascii_case("auto") {
            return Some(MarginValue {
                length: AUTO_LENGTH,
                auto: true,
            });
        }
        self.box_length(value, true).map(|length| MarginValue {
            length,
            auto: false,
        })
    }

    pub(crate) fn box_length(&self, value: &str, signed: bool) -> Option<f32> {
        css_resolved_px(
            value,
            self.font_size,
            self.viewport_width,
            self.viewport_height,
            signed,
        )
    }
}

const BOX_EDGE_COUNT: usize = 4;

fn expand_margin_values(values: Vec<MarginValue>) -> Option<[MarginValue; BOX_EDGE_COUNT]> {
    expand_box_values(&values)
}

#[cfg(test)]
mod tests {
    use super::MarginValue;
    use super::expand_box_values;
    use super::expand_margin_values;

    type BoxShorthandCase<'a> = (&'a [i32], Option<[i32; 4]>);

    #[test]
    fn box_shorthand_expands_zero_through_five_values() {
        let cases: [BoxShorthandCase<'_>; 6] = [
            (&[], None),
            (&[1], Some([1, 1, 1, 1])),
            (&[1, 2], Some([1, 2, 1, 2])),
            (&[1, 2, 3], Some([1, 2, 3, 2])),
            (&[1, 2, 3, 4], Some([1, 2, 3, 4])),
            (&[1, 2, 3, 4, 5], None),
        ];
        for (values, expected) in cases {
            assert_eq!(expand_box_values(values), expected);
        }
    }

    fn margin(length: f32) -> MarginValue {
        MarginValue {
            length,
            auto: false,
        }
    }

    fn must_expand(values: Vec<MarginValue>) -> [MarginValue; 4] {
        let expanded = expand_margin_values(values);
        assert!(expanded.is_some());
        let mut values = expanded.into_iter().collect::<Vec<_>>();
        values.remove(0)
    }

    #[test]
    fn two_value_shorthand_expands_to_horizontal_and_vertical_pairs() {
        let resolved = must_expand(vec![margin(1.0), margin(2.0)]);

        assert_eq!(resolved[0].length, 1.0);
        assert_eq!(resolved[1].length, 2.0);
        assert_eq!(resolved[2].length, 1.0);
        assert_eq!(resolved[3].length, 2.0);
        assert!(!resolved[0].auto && !resolved[1].auto && !resolved[2].auto && !resolved[3].auto);
    }

    #[test]
    fn three_value_shorthand_expands_to_top_horizontal_and_bottom() {
        let resolved = must_expand(vec![margin(1.0), margin(2.0), margin(3.0)]);

        assert_eq!(resolved[0].length, 1.0);
        assert_eq!(resolved[1].length, 2.0);
        assert_eq!(resolved[2].length, 3.0);
        assert_eq!(resolved[3].length, 2.0);
        assert!(!resolved[0].auto && !resolved[1].auto && !resolved[2].auto && !resolved[3].auto);
    }

    #[test]
    fn applies_margin_shorthand_to_all_sides_when_single_value_is_given() {
        let resolved = must_expand(vec![margin(5.0)]);

        assert_eq!(resolved[0].length, 5.0);
        assert_eq!(resolved[1].length, 5.0);
        assert_eq!(resolved[2].length, 5.0);
        assert_eq!(resolved[3].length, 5.0);
        assert!(!resolved[0].auto && !resolved[1].auto && !resolved[2].auto && !resolved[3].auto);
    }
}
