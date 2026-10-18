use super::{HtmlBrowserError, HtmlBrowserViewport};
use serde::{Deserialize, Serialize};

/// CSS pixel単位のroot overflow policyを表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlBrowserOverflowPolicy {
    Auto,
    Scroll,
    Hidden,
    Clip,
}

/// frameと同じgenerationに属するCSS pixel単位のscroll metrics。
/// viewportのwidth/heightはphysical pixel frameからdevice scale factorで論理化される。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HtmlBrowserScrollMetrics {
    pub generation: u64,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub content_width: f32,
    pub content_height: f32,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub overflow_x: HtmlBrowserOverflowPolicy,
    pub overflow_y: HtmlBrowserOverflowPolicy,
}

impl HtmlBrowserScrollMetrics {
    pub(crate) fn new(
        generation: u64,
        viewport: HtmlBrowserViewport,
        content: (f32, f32),
        offset: (f32, f32),
        overflow: (HtmlBrowserOverflowPolicy, HtmlBrowserOverflowPolicy),
    ) -> Result<Self, HtmlBrowserError> {
        let (viewport_width, viewport_height) = validated_viewport(viewport)?;
        validate_content(content, (viewport_width, viewport_height))?;
        validate_offset(offset)?;
        let (content_width, content_height) = content;
        let (scroll_x, scroll_y) = offset;
        let metrics = Self {
            generation,
            viewport_width,
            viewport_height,
            content_width,
            content_height,
            scroll_x,
            scroll_y,
            overflow_x: overflow.0,
            overflow_y: overflow.1,
        };
        validate_scroll_bounds(&metrics)?;
        Ok(metrics)
    }

    pub fn max_scroll_x(&self) -> f32 {
        match self.overflow_x {
            HtmlBrowserOverflowPolicy::Clip => 0.0,
            _ => (self.content_width.round() - self.viewport_width).max(0.0),
        }
    }

    pub fn max_scroll_y(&self) -> f32 {
        match self.overflow_y {
            HtmlBrowserOverflowPolicy::Clip => 0.0,
            _ => (self.content_height.round() - self.viewport_height).max(0.0),
        }
    }

    pub fn horizontal_scrollbar_visible(&self) -> bool {
        match self.overflow_x {
            HtmlBrowserOverflowPolicy::Scroll => true,
            HtmlBrowserOverflowPolicy::Auto => self.max_scroll_x() > 0.0,
            HtmlBrowserOverflowPolicy::Hidden | HtmlBrowserOverflowPolicy::Clip => false,
        }
    }

    pub fn vertical_scrollbar_visible(&self) -> bool {
        match self.overflow_y {
            HtmlBrowserOverflowPolicy::Scroll => true,
            HtmlBrowserOverflowPolicy::Auto => self.max_scroll_y() > 0.0,
            HtmlBrowserOverflowPolicy::Hidden | HtmlBrowserOverflowPolicy::Clip => false,
        }
    }
}

fn validated_viewport(viewport: HtmlBrowserViewport) -> Result<(f32, f32), HtmlBrowserError> {
    viewport.validate()?;
    Ok((viewport.logical_width(), viewport.logical_height()))
}

fn validate_content(
    (content_width, content_height): (f32, f32),
    (viewport_width, viewport_height): (f32, f32),
) -> Result<(), HtmlBrowserError> {
    if !content_width.is_finite()
        || !content_height.is_finite()
        || content_width < 0.0
        || content_height < 0.0
        || content_width < viewport_width
        || content_height < viewport_height
    {
        Err(HtmlBrowserError::InvalidViewport)
    } else {
        Ok(())
    }
}

fn validate_offset((scroll_x, scroll_y): (f32, f32)) -> Result<(), HtmlBrowserError> {
    if !scroll_x.is_finite() || !scroll_y.is_finite() || scroll_x < 0.0 || scroll_y < 0.0 {
        Err(HtmlBrowserError::InvalidInputCoordinates)
    } else {
        Ok(())
    }
}

fn validate_scroll_bounds(metrics: &HtmlBrowserScrollMetrics) -> Result<(), HtmlBrowserError> {
    if metrics.scroll_x > metrics.max_scroll_x() || metrics.scroll_y > metrics.max_scroll_y() {
        Err(HtmlBrowserError::InvalidInputCoordinates)
    } else {
        Ok(())
    }
}
