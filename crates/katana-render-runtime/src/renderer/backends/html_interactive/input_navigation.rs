use percent_encoding::percent_decode_str;

use super::super::super::html_browser::{HtmlBrowserNavigationEvent, HtmlBrowserOrigin};
use super::super::runtime_failure;
use super::super::session_geometry::max_scroll_for;
use super::super::types::LayoutResult;
use super::super::{HtmlBrowserError, HtmlInteractiveSession};

impl HtmlInteractiveSession {
    pub(crate) fn accept_navigation(
        &mut self,
        dispatch: super::super::super::html_runtime::HtmlRuntimeDispatch,
    ) -> Result<(), HtmlBrowserError> {
        let Some(intent) = dispatch.navigation else {
            return Ok(());
        };
        let url = self
            .resource_policy
            .resolve_navigation(&intent.href)
            .map_err(navigation_error)?;
        if self
            .source
            .origin
            .is_same_document_fragment_navigation(&url)
        {
            self.apply_fragment_navigation(url)?;
        } else {
            self.pending_navigation = Some(HtmlBrowserNavigationEvent { url });
        }
        Ok(())
    }

    pub(crate) fn apply_fragment_navigation(
        &mut self,
        url: HtmlBrowserOrigin,
    ) -> Result<(), HtmlBrowserError> {
        self.apply_fragment_navigation_in_layout(url, None)
    }

    pub(in crate::renderer::backends::html_interactive) fn apply_fragment_navigation_in_layout(
        &mut self,
        url: HtmlBrowserOrigin,
        layout: Option<&LayoutResult>,
    ) -> Result<(), HtmlBrowserError> {
        let fragment = url
            .url()
            .fragment()
            .map(|value| percent_decode_str(value).decode_utf8_lossy().into_owned());
        let (next_scroll, resize_anchor) = match fragment.as_deref() {
            None | Some("") => (0.0, None),
            Some(fragment) => match self.fragment_position(fragment, layout)? {
                Some(position) => (position.round(), Some(fragment.to_string())),
                None => (self.scroll_geometry.scroll_y, None),
            },
        };
        self.runtime
            .update_document_url(url.as_str())
            .map_err(runtime_failure)?;
        self.source.origin = url;
        let limit = layout.map_or_else(
            || self.max_scroll(),
            |layout| max_scroll_for(layout, self.viewport),
        );
        self.scroll_geometry.scroll_y = next_scroll.clamp(0.0, limit);
        self.resize_anchor = resize_anchor;
        Ok(())
    }

    fn fragment_position(
        &mut self,
        fragment: &str,
        layout: Option<&LayoutResult>,
    ) -> Result<Option<f32>, HtmlBrowserError> {
        match layout {
            Some(layout) => Ok(layout.anchor_positions.get(fragment).copied()),
            None => Ok(self.layout()?.anchor_positions.get(fragment).copied()),
        }
    }
}

fn navigation_error(error: String) -> HtmlBrowserError {
    if error.starts_with("resource URL is invalid") {
        return runtime_failure(format!("link target is invalid: {error}"));
    }
    runtime_failure(error)
}
