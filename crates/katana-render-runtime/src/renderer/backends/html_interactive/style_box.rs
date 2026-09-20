use super::super::constants::BORDER_RADIUS_CORNER_COUNT;
use super::{CssBoxSizing, CssOverflow, CssStyle};

impl CssStyle {
    pub(in super::super) fn explicit_width(&self, available: f32) -> Option<f32> {
        self.width.map(|width| {
            let resolved = self.outer_width(width.resolve(available));
            let resolved = self.max_width.map_or(resolved, |maximum| {
                resolved.min(self.outer_width(maximum.resolve(available)))
            });
            self.minimum_outer_width(available)
                .map_or(resolved, |minimum| resolved.max(minimum))
        })
    }

    pub(in super::super) fn box_width(&self, available: f32) -> f32 {
        let width = self.explicit_width(available).unwrap_or(available);
        let width = self.max_width.map_or(width, |maximum| {
            width.min(self.outer_width(maximum.resolve(available)))
        });
        self.minimum_outer_width(available)
            .map_or(width, |minimum| width.max(minimum))
    }

    pub(in super::super) fn minimum_outer_width(&self, available: f32) -> Option<f32> {
        self.min_width
            .map(|minimum| self.outer_width(minimum.resolve(available)))
    }

    pub(in super::super) fn content_width(&self, outer_width: f32) -> f32 {
        (outer_width - self.horizontal_non_content()).max(0.0)
    }

    pub(in super::super) fn outer_height(&self, css_height: f32) -> f32 {
        match self.box_sizing {
            CssBoxSizing::ContentBox => css_height + self.vertical_non_content(),
            CssBoxSizing::BorderBox => css_height,
        }
    }

    pub(in super::super) fn content_height(&self, outer_height: f32) -> f32 {
        (outer_height - self.vertical_non_content()).max(0.0)
    }

    pub(in super::super) fn children_height(&self) -> Option<f32> {
        self.height.map(|height| match self.box_sizing {
            CssBoxSizing::ContentBox => height,
            CssBoxSizing::BorderBox => self.content_height(height),
        })
    }

    pub(in super::super) fn minimum_outer_height(&self) -> f32 {
        self.outer_height(self.min_height)
    }

    pub(in super::super) fn clips_overflow(&self) -> bool {
        self.overflow == CssOverflow::Clip
    }

    pub(in super::super) fn border_top_width(&self) -> f32 {
        self.border_top_width.unwrap_or(self.border_width)
    }

    pub(in super::super) fn border_right_width(&self) -> f32 {
        self.border_right_width.unwrap_or(self.border_width)
    }

    pub(in super::super) fn border_bottom_width(&self) -> f32 {
        self.border_bottom_width.unwrap_or(self.border_width)
    }

    pub(in super::super) fn border_left_width(&self) -> f32 {
        self.border_left_width.unwrap_or(self.border_width)
    }

    pub(in super::super) fn border_top_color(&self) -> Option<&str> {
        self.border_top_color.as_deref().or(self.border.as_deref())
    }

    pub(in super::super) fn border_right_color(&self) -> Option<&str> {
        self.border_right_color
            .as_deref()
            .or(self.border.as_deref())
    }

    pub(in super::super) fn border_bottom_color(&self) -> Option<&str> {
        self.border_bottom_color
            .as_deref()
            .or(self.border.as_deref())
    }

    pub(in super::super) fn border_left_color(&self) -> Option<&str> {
        self.border_left_color.as_deref().or(self.border.as_deref())
    }

    pub(in super::super) fn has_border_edge_overrides(&self) -> bool {
        self.border_top_width.is_some()
            || self.border_right_width.is_some()
            || self.border_bottom_width.is_some()
            || self.border_left_width.is_some()
            || self.border_top_color.is_some()
            || self.border_right_color.is_some()
            || self.border_bottom_color.is_some()
            || self.border_left_color.is_some()
    }

    pub(in super::super) fn has_any_border(&self) -> bool {
        self.border_top_color().is_some()
            || self.border_right_color().is_some()
            || self.border_bottom_color().is_some()
            || self.border_left_color().is_some()
    }

    pub(in super::super) fn resolved_border_radius(&self, width: f32, height: f32) -> (f32, f32) {
        let (horizontal, vertical) = match self.border_radius {
            super::CssLength::Px(value) => (value, value),
            super::CssLength::Percent(factor) => (width * factor, height * factor),
        };
        (
            horizontal.min(width / 2.0).max(0.0),
            vertical.min(height / 2.0).max(0.0),
        )
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

    pub(in super::super) fn assign_outer_width(&mut self, outer_width: f32) {
        self.width = Some(super::CssLength::Px(match self.box_sizing {
            CssBoxSizing::ContentBox => self.content_width(outer_width),
            CssBoxSizing::BorderBox => outer_width,
        }));
        self.min_width = None;
        self.max_width = None;
    }

    pub(in super::super) fn assign_outer_height(&mut self, outer_height: f32) {
        self.height = Some(match self.box_sizing {
            CssBoxSizing::ContentBox => self.content_height(outer_height),
            CssBoxSizing::BorderBox => outer_height,
        });
        self.max_height = None;
    }

    pub(in super::super) fn assign_margin_box_height(&mut self, margin_box_height: f32) {
        let outer_height = (margin_box_height - self.margin_top - self.margin_bottom).max(0.0);
        self.assign_outer_height(outer_height);
    }

    pub(in super::super) fn outer_width(&self, css_width: f32) -> f32 {
        match self.box_sizing {
            CssBoxSizing::ContentBox => css_width + self.horizontal_non_content(),
            CssBoxSizing::BorderBox => css_width,
        }
    }

    pub(in super::super) fn intrinsic_outer_width(&self, content_width: f32) -> f32 {
        content_width + self.horizontal_non_content()
    }

    pub(in super::super) fn flex_basis_outer_width(&self, css_basis: f32) -> f32 {
        self.outer_width(css_basis)
            .max(self.horizontal_non_content())
    }

    fn horizontal_non_content(&self) -> f32 {
        self.padding_left
            + self.padding_right
            + self.border_left_width()
            + self.border_right_width()
    }

    fn vertical_non_content(&self) -> f32 {
        self.padding_top
            + self.padding_bottom
            + self.border_top_width()
            + self.border_bottom_width()
    }
}

#[cfg(test)]
mod tests {
    use super::super::CssLength;
    use super::CssStyle;

    #[test]
    fn explicit_and_automatic_widths_are_clamped_by_min_width() {
        let mut style = CssStyle::browser_default();
        style.width = Some(CssLength::Px(20.0));
        style.min_width = Some(CssLength::Px(40.0));

        assert_eq!(style.explicit_width(100.0), Some(40.0));

        style.width = None;
        assert_eq!(style.box_width(30.0), 40.0);
    }
}
