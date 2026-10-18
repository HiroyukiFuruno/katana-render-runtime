use percent_encoding::percent_decode_str;

use super::super::super::html_browser::HtmlBrowserNavigationEvent;
use super::super::runtime_failure;
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
        url: super::super::super::html_browser::HtmlBrowserOrigin,
    ) -> Result<(), HtmlBrowserError> {
        let fragment = url
            .url()
            .fragment()
            .map(|value| percent_decode_str(value).decode_utf8_lossy().into_owned());
        let (next_scroll, resize_anchor) = match fragment.as_deref() {
            None | Some("") => (0.0, None),
            Some(fragment) => match self.layout()?.anchor_positions.get(fragment).copied() {
                Some(position) => (position.round(), Some(fragment.to_string())),
                None => (self.scroll_geometry.scroll_y, None),
            },
        };
        self.runtime
            .update_document_url(url.as_str())
            .map_err(runtime_failure)?;
        self.source.origin = url;
        self.scroll_geometry.scroll_y = next_scroll.clamp(0.0, self.max_scroll());
        self.resize_anchor = resize_anchor;
        Ok(())
    }
}

fn navigation_error(error: String) -> HtmlBrowserError {
    if error.starts_with("resource URL is invalid") {
        return runtime_failure(format!("link target is invalid: {error}"));
    }
    runtime_failure(error)
}
