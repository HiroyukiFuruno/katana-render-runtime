#[path = "layout_inline_fragment.rs"]
mod fragment;
#[path = "layout_inline_fragments.rs"]
mod fragments;
#[path = "layout_inline_measure.rs"]
mod measure;
#[path = "layout_inline_paint.rs"]
mod paint;

#[path = "layout_inline_floats.rs"]
mod floats;
#[path = "layout_inline_render.rs"]
mod render;
#[path = "layout_inline_state.rs"]
mod state;

#[cfg(test)]
#[path = "layout_inline_tests.rs"]
mod tests;

use fragment::{advance_inline_text, inline_flow_style};
pub(super) use measure::InlineMeasurement;
use state::InlineFlowState;

#[cfg(test)]
pub(super) use super::super::html_document::HtmlDocumentNode;
#[cfg(test)]
pub(super) use super::layout::HtmlLayoutRenderer;
#[cfg(test)]
pub(super) use super::style::{CssFloat, CssStyle};
#[cfg(test)]
pub(super) use floats::InlineFloat;
