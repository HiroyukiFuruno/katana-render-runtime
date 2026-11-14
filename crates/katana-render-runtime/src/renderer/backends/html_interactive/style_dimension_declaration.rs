use super::value::{expand_box_values, split_top_level_whitespace};
use super::{CssLength, CssStyle};

const INSET_EDGE_COUNT: usize = 4;

impl CssStyle {
    pub(super) fn apply_dimensions(&mut self, name: &str, value: &str) {
        match name {
            "width" | "min-width" | "max-width" => self.apply_width_dimension(name, value),
            "height" => {
                self.explicit_height_auto = value.trim().eq_ignore_ascii_case("auto");
                self.height = self.block_length(value);
            }
            "min-height" => self.apply_minimum_height(value),
            "max-height" => self.max_height = self.block_length(value),
            _ => {}
        }
    }

    fn apply_width_dimension(&mut self, name: &str, value: &str) {
        let resolved = CssLength::parse(
            value,
            self.font_size,
            self.viewport_width,
            self.viewport_height,
        );
        match name {
            "width" => {
                self.explicit_width = resolved.is_some();
                self.width = resolved;
            }
            "min-width" => self.min_width = resolved,
            "max-width" => self.max_width = resolved,
            _ => {}
        }
    }

    fn apply_minimum_height(&mut self, value: &str) {
        if value.trim().eq_ignore_ascii_case("auto") {
            self.min_height = 0.0;
            self.automatic_min_height = true;
        } else if let Some(minimum) = self.block_length(value) {
            self.min_height = minimum;
            self.automatic_min_height = false;
        }
    }

    pub(super) fn apply_inset(&mut self, name: &str, value: &str) {
        if name == "inset" {
            let Some([top, right, bottom, left]) = self.inset_sides(value) else {
                return;
            };
            self.inset_top = top;
            self.inset_right = right;
            self.inset_bottom = bottom;
            self.inset_left = left;
            return;
        }
        let Some(resolved) = self.inset_value(value) else {
            return;
        };
        match name {
            "top" => self.inset_top = resolved,
            "right" => self.inset_right = resolved,
            "bottom" => self.inset_bottom = resolved,
            "left" => self.inset_left = resolved,
            _ => {}
        }
    }

    fn inset_sides(&self, value: &str) -> Option<[Option<f32>; INSET_EDGE_COUNT]> {
        let values = split_top_level_whitespace(value)
            .into_iter()
            .map(|value| self.inset_value(value))
            .collect::<Option<Vec<_>>>()?;
        expand_box_values(&values)
    }

    fn inset_value(&self, value: &str) -> Option<Option<f32>> {
        if value.trim().eq_ignore_ascii_case("auto") {
            Some(None)
        } else {
            self.box_length(value, true).map(Some)
        }
    }

    fn block_length(&self, value: &str) -> Option<f32> {
        if let Some(percent) = value.trim().strip_suffix('%') {
            let percent = percent.trim().parse::<f32>().ok()?;
            if !percent.is_finite() || percent < 0.0 {
                return None;
            }
            return self
                .percentage_height_basis
                .map(|basis| basis * percent / 100.0);
        }
        self.box_length(value, false)
    }
}

#[cfg(test)]
mod tests {
    use super::CssStyle;

    #[test]
    fn dimensions_ignore_unknown_names_and_restore_auto_minimum_height() {
        let mut style = CssStyle::browser_default();
        style.apply_width_dimension("unknown", "12px");
        style.min_height = 10.0;

        style.apply_minimum_height("auto");

        assert_eq!(style.min_height, 0.0);
        assert!(style.automatic_min_height);
    }

    #[test]
    fn inset_ignores_invalid_shorthand_and_unknown_longhand() {
        let mut style = CssStyle::browser_default();
        style.apply_inset("inset", "1px 2px 3px 4px 5px");
        style.apply_inset("inline-start", "9px");

        assert!(style.inset_top.is_none());
        assert!(style.inset_left.is_none());
    }

    #[test]
    fn inset_shorthand_expands_auto_and_signed_lengths_atomically() {
        for (value, expected) in [
            ("auto", [None; 4]),
            ("0 auto", [Some(0.0), None, Some(0.0), None]),
            ("auto -2px 3px", [None, Some(-2.0), Some(3.0), Some(-2.0)]),
            ("0 auto 0 0", [Some(0.0), None, Some(0.0), Some(0.0)]),
        ] {
            let mut style = CssStyle::browser_default();
            style.apply_inset("inset", value);
            assert_eq!(insets(&style), expected, "{value}");
            style.apply_inset("inset", "0 auto invalid 0");
            assert_eq!(
                insets(&style),
                expected,
                "invalid shorthand must preserve {value}"
            );
        }
    }

    #[test]
    fn inset_longhand_preserves_invalid_values_and_resets_auto() {
        let mut style = CssStyle::browser_default();
        for name in ["top", "right", "bottom", "left"] {
            style.apply_inset(name, "12px");
            style.apply_inset(name, "invalid");
        }
        assert_eq!(insets(&style), [Some(12.0); 4]);
        for name in ["top", "right", "bottom", "left"] {
            style.apply_inset(name, "AUTO");
        }
        assert_eq!(insets(&style), [None; 4]);
    }

    fn insets(style: &CssStyle) -> [Option<f32>; 4] {
        [
            style.inset_top,
            style.inset_right,
            style.inset_bottom,
            style.inset_left,
        ]
    }

    #[test]
    fn percentage_block_length_rejects_negative_and_non_finite_values() {
        let style = CssStyle::browser_default();

        assert!(style.block_length("-1%").is_none());
        assert!(style.block_length("NaN%").is_none());
    }

    #[test]
    fn minimum_height_updates_value_when_length_is_specified() {
        let mut style = CssStyle::browser_default();
        style.automatic_min_height = true;
        style.min_height = 5.0;

        style.apply_minimum_height("10px");

        assert_eq!(style.min_height, 10.0);
        assert!(!style.automatic_min_height);

        style.apply_minimum_height("invalid");
        assert_eq!(style.min_height, 10.0);
        assert!(!style.automatic_min_height);
    }
}
