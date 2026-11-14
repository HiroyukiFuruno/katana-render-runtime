use super::super::document::css_px;
use super::super::style::value::grid_tracks;
use super::CssStyle;

impl CssStyle {
    pub(super) fn apply_multicolumn_property(&mut self, name: &str, value: &str) -> bool {
        match name {
            "grid-template-columns" => {
                self.grid_template_columns = grid_tracks(value, self.font_size)
                    .unwrap_or_else(|| self.grid_template_columns.clone());
            }
            "column-count" => self.apply_column_count(value),
            "column-width" => self.apply_column_width(value),
            "column-gap" => self.apply_column_gap(value),
            "columns" => self.apply_columns(value),
            "break-inside" => {
                self.break_inside_avoid = value.trim().eq_ignore_ascii_case("avoid");
            }
            _ => return false,
        }
        true
    }

    fn apply_column_count(&mut self, value: &str) {
        if let Ok(count) = value.trim().parse::<usize>()
            && count > 0
        {
            self.column_count = Some(count);
        }
    }

    fn apply_column_width(&mut self, value: &str) {
        if let Some(width) = css_px(value)
            && width.is_finite()
            && width > 0.0
        {
            self.column_width = Some(width);
        }
    }

    fn apply_column_gap(&mut self, value: &str) {
        if value.trim().eq_ignore_ascii_case("normal") {
            self.column_gap = self.font_size;
            self.flow_column_gap = Some(0.0);
            return;
        }
        if let Some(gap) = css_px(value)
            && gap.is_finite()
            && gap >= 0.0
        {
            self.column_gap = gap;
            self.flow_column_gap = Some(gap);
        }
    }

    pub(super) fn apply_columns(&mut self, value: &str) {
        for token in value.split_whitespace() {
            if let Ok(count) = token.parse::<usize>()
                && count > 0
            {
                self.column_count = Some(count);
            } else if let Some(width) = css_px(token)
                && width.is_finite()
                && width > 0.0
            {
                self.column_width = Some(width);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CssStyle;

    #[test]
    fn column_longhands_accept_positive_values_and_preserve_values_on_invalid_input() {
        let mut style = CssStyle::browser_default();
        assert!(style.apply_multicolumn_property("column-count", " 3 "));
        assert!(style.apply_multicolumn_property("column-width", "120px"));
        assert_eq!(style.column_count, Some(3));
        assert_eq!(style.column_width, Some(120.0));
        assert_invalid_column_values_preserve(&mut style);
    }

    fn assert_invalid_column_values_preserve(style: &mut CssStyle) {
        for value in ["0", "-2", "not-a-count"] {
            assert!(style.apply_multicolumn_property("column-count", value));
            assert_eq!(style.column_count, Some(3));
        }
        for value in ["0px", "-2px", "NaNpx", "infpx", "invalid"] {
            assert!(style.apply_multicolumn_property("column-width", value));
            assert_eq!(style.column_width, Some(120.0));
        }
    }
}
