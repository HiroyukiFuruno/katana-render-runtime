#[path = "html_interactive/layout_axis_clip.rs"]
mod axis_clip;
mod constants;
mod control_style;
mod document;
mod input;
mod input_focus;
mod layout;
mod layout_block_margin;
mod layout_clip;
mod layout_container;
mod layout_control_paint;
mod layout_controls;
mod layout_dispatch;
mod layout_element;
mod layout_flow;
mod layout_flow_item;
mod layout_flow_measure;
mod layout_grid_measure;
mod layout_grid_track;
mod layout_inline;
mod layout_input;
mod layout_measurement_cache;
mod layout_media;
mod layout_paint;
mod layout_paint_lines;
mod layout_paint_order;
mod layout_position;
mod layout_render;
mod layout_scroll_extent;
mod layout_select;
mod layout_sticky;
mod layout_structures;
mod layout_summary;
mod layout_table;
mod layout_text;
mod layout_viewport_overflow;
mod session;
mod session_geometry;
mod session_render;
mod session_render_layout;
mod session_scroll_geometry;
mod style;
mod style_declaration_cache;
mod svg;
#[cfg(test)]
mod tests;
mod text_measurement_diagnostic;
mod text_metrics;
mod types;

use super::html_browser::HtmlBrowserError;

pub(super) use session::HtmlInteractiveSession;

fn runtime_failure(error: impl ToString) -> HtmlBrowserError {
    HtmlBrowserError::RuntimeFailure {
        error: error.to_string(),
    }
}

#[cfg(test)]
mod runtime_failure_tests {
    use super::runtime_failure;
    use crate::renderer::backends::html_browser::HtmlBrowserError;

    #[test]
    fn runtime_failure_keeps_the_underlying_message() {
        assert_eq!(
            runtime_failure("raster failed"),
            HtmlBrowserError::RuntimeFailure {
                error: "raster failed".to_string(),
            }
        );
    }
}
