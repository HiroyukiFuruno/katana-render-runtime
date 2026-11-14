use super::super::constants::BORDER_RADIUS_CORNER_COUNT;
use super::CssStyle;

impl CssStyle {
    pub(in super::super) fn resolved_border_radius(&self, width: f32, height: f32) -> (f32, f32) {
        let (horizontal, vertical) = match self.border_radius {
            super::CssLength::Px(value) => (value, value),
            super::CssLength::Percent(factor) => (width * factor, height * factor),
        };
        let horizontal = horizontal.max(0.0);
        let vertical = vertical.max(0.0);
        let mut factor = 1.0_f32;
        if horizontal > 0.0 {
            factor = factor.min(width / (2.0 * horizontal));
        }
        if vertical > 0.0 {
            factor = factor.min(height / (2.0 * vertical));
        }
        (horizontal * factor.max(0.0), vertical * factor.max(0.0))
    }

    pub(in super::super) fn resolved_inner_border_radii(
        &self,
        width: f32,
        height: f32,
    ) -> [(f32, f32); BORDER_RADIUS_CORNER_COUNT] {
        let (horizontal, vertical) = self.resolved_border_radius(width, height);
        let left = self.border_left_width();
        let right = self.border_right_width();
        let top = self.border_top_width();
        let bottom = self.border_bottom_width();
        [
            ((horizontal - left).max(0.0), (vertical - top).max(0.0)),
            ((horizontal - right).max(0.0), (vertical - top).max(0.0)),
            ((horizontal - right).max(0.0), (vertical - bottom).max(0.0)),
            ((horizontal - left).max(0.0), (vertical - bottom).max(0.0)),
        ]
    }
}
