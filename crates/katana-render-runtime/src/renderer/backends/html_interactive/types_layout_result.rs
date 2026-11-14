use super::{ElementBox, HitTarget};
use crate::renderer::backends::HtmlBrowserOverflowPolicy;
use std::collections::HashMap;

pub(in crate::renderer::backends::html_interactive) struct LayoutResult {
    pub(in crate::renderer::backends::html_interactive) svg: String,
    pub(in crate::renderer::backends::html_interactive) generated_clip_provenance:
        crate::markdown::svg_rasterize::GeneratedClipProvenance,
    pub(in crate::renderer::backends::html_interactive) generated_html_text_provenance:
        crate::markdown::svg_rasterize::GeneratedHtmlTextProvenance,
    pub(in crate::renderer::backends::html_interactive) hit_targets: Vec<HitTarget>,
    pub(in crate::renderer::backends::html_interactive) element_boxes: Vec<ElementBox>,
    pub(in crate::renderer::backends::html_interactive) anchor_positions: HashMap<String, f32>,
    pub(in crate::renderer::backends::html_interactive) content_width: f32,
    pub(in crate::renderer::backends::html_interactive) content_height: f32,
    pub(in crate::renderer::backends::html_interactive) viewport_overflow:
        (HtmlBrowserOverflowPolicy, HtmlBrowserOverflowPolicy),
}
