use super::HtmlBrowserError;
use serde::{Deserialize, Serialize};
use url::Url;

pub const HTML_BROWSER_MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HtmlBrowserOrigin(Url);

impl HtmlBrowserOrigin {
    pub fn parse(value: impl Into<String>) -> Result<Self, HtmlBrowserError> {
        let value = value.into();
        let parsed = Url::parse(&value).map_err(|_| HtmlBrowserError::InvalidOrigin {
            origin: value.clone(),
        })?;
        Self::validate_url(&parsed, &value)?;
        Ok(Self(parsed))
    }

    fn validate_url(parsed: &Url, original: &str) -> Result<(), HtmlBrowserError> {
        match parsed.scheme() {
            "file" if parsed.to_file_path().is_ok() => {}
            "http" | "https" if parsed.host_str().is_some() => {}
            _ => {
                return Err(HtmlBrowserError::UnsupportedOriginScheme {
                    origin: original.to_owned(),
                });
            }
        }
        Ok(())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub(in crate::renderer::backends) fn url(&self) -> &Url {
        &self.0
    }

    pub(in crate::renderer::backends) fn from_validated_url(url: Url) -> Self {
        Self(url)
    }

    pub(in crate::renderer::backends) fn is_same_document_fragment_navigation(
        &self,
        target: &Self,
    ) -> bool {
        if self.0.fragment().is_none() && target.0.fragment().is_none() {
            return false;
        }
        let mut current = self.0.clone();
        let mut target = target.0.clone();
        current.set_fragment(None);
        target.set_fragment(None);
        current == target
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HtmlBrowserSource {
    pub raw_html: String,
    pub origin: HtmlBrowserOrigin,
}

impl HtmlBrowserSource {
    pub fn new(
        raw_html: impl Into<String>,
        origin: impl Into<String>,
    ) -> Result<Self, HtmlBrowserError> {
        let raw_html = raw_html.into();
        Self::validate_size(&raw_html)?;
        Ok(Self {
            raw_html,
            origin: HtmlBrowserOrigin::parse(origin)?,
        })
    }

    pub fn validate(&self) -> Result<(), HtmlBrowserError> {
        Self::validate_size(&self.raw_html)?;
        HtmlBrowserOrigin::validate_url(&self.origin.0, self.origin.as_str())
    }

    fn validate_size(raw_html: &str) -> Result<(), HtmlBrowserError> {
        if raw_html.len() > HTML_BROWSER_MAX_SOURCE_BYTES {
            return Err(HtmlBrowserError::SourceTooLarge {
                actual_bytes: raw_html.len(),
                max_bytes: HTML_BROWSER_MAX_SOURCE_BYTES,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HtmlBrowserViewport {
    pub width: u32,
    pub height: u32,
    pub device_scale_factor: f32,
}

impl HtmlBrowserViewport {
    pub fn new(
        width: u32,
        height: u32,
        device_scale_factor: f32,
    ) -> Result<Self, HtmlBrowserError> {
        let viewport = Self {
            width,
            height,
            device_scale_factor,
        };
        viewport.validate()?;
        Ok(viewport)
    }

    pub fn validate(&self) -> Result<(), HtmlBrowserError> {
        if self.width == 0 || self.height == 0 {
            return Err(HtmlBrowserError::InvalidViewport);
        }
        if !self.device_scale_factor.is_finite() || self.device_scale_factor <= 0.0 {
            return Err(HtmlBrowserError::InvalidDeviceScaleFactor);
        }
        Ok(())
    }

    pub fn logical_width(&self) -> f32 {
        self.width as f32 / self.device_scale_factor
    }

    pub fn logical_height(&self) -> f32 {
        self.height as f32 / self.device_scale_factor
    }
}

/// ポインターのviewport座標とスクロール差分は物理pixelで渡す。
/// 内部でdevice scale factorで割り、返却されるスクロールmetricsはCSS pixel単位となる。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HtmlBrowserInput {
    Focus { focused: bool },
    PointerMove { x: f32, y: f32 },
    PointerDown { x: f32, y: f32, button: u8 },
    PointerUp { x: f32, y: f32, button: u8 },
    Scroll { delta_x: f32, delta_y: f32 },
    KeyDown { key: String },
    KeyUp { key: String },
    Text { text: String },
}

impl HtmlBrowserInput {
    pub fn validate(&self) -> Result<(), HtmlBrowserError> {
        let finite = |value: f32| value.is_finite();
        let valid = match self {
            Self::PointerMove { x, y }
            | Self::PointerDown { x, y, .. }
            | Self::PointerUp { x, y, .. } => finite(*x) && finite(*y),
            Self::Scroll { delta_x, delta_y } => finite(*delta_x) && finite(*delta_y),
            Self::Focus { .. } | Self::KeyDown { .. } | Self::KeyUp { .. } | Self::Text { .. } => {
                true
            }
        };
        valid
            .then_some(())
            .ok_or(HtmlBrowserError::InvalidInputCoordinates)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HtmlBrowserNavigation {
    pub source: HtmlBrowserSource,
}

impl HtmlBrowserNavigation {
    pub fn new(source: HtmlBrowserSource) -> Result<Self, HtmlBrowserError> {
        source.validate()?;
        Ok(Self { source })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HtmlBrowserNavigationEvent {
    pub url: HtmlBrowserOrigin,
}

impl HtmlBrowserNavigationEvent {
    pub fn new(url: impl Into<String>) -> Result<Self, HtmlBrowserError> {
        Ok(Self {
            url: HtmlBrowserOrigin::parse(url)?,
        })
    }
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
