use crate::renderer::backends::html_interactive::document::css_px;
use crate::renderer::backends::html_interactive::style::CssStyle;

impl CssStyle {
    pub(super) fn apply_gap(&mut self, value: &str) {
        if value.trim().eq_ignore_ascii_case("normal") {
            self.gap = 0.0;
            self.row_gap = Some(0.0);
            self.flow_column_gap = Some(0.0);
            return;
        }
        let values = value
            .split_whitespace()
            .map(css_px)
            .collect::<Option<Vec<_>>>();
        let Some(values) = values else {
            return;
        };
        match values.as_slice() {
            [row] => self.set_gap_values(*row, *row),
            [row, column] => self.set_gap_values(*row, *column),
            _ => {}
        }
    }

    fn set_gap_values(&mut self, row: f32, column: f32) {
        self.gap = row;
        self.row_gap = Some(row);
        self.flow_column_gap = Some(column);
    }

    pub(super) fn apply_row_gap(&mut self, value: &str) {
        if value.trim().eq_ignore_ascii_case("normal") {
            self.row_gap = Some(0.0);
        } else if let Some(gap) = css_px(value) {
            self.row_gap = Some(gap);
        }
    }
}
