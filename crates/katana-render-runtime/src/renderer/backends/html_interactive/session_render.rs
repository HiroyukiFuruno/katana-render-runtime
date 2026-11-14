use super::super::html_browser::{
    HtmlBrowserFrame, HtmlBrowserPixelFormat, HtmlBrowserScrollMetrics,
};
use super::session::HtmlInteractiveSession;
use super::session_geometry::max_scroll_for;
use super::types::LayoutResult;
use super::{HtmlBrowserError, runtime_failure};
use crate::markdown::svg_rasterize::SvgRasterizeOps;

impl HtmlInteractiveSession {
    pub(super) fn render_frame(&mut self) -> Result<(), HtmlBrowserError> {
        self.render_frame_with_resize_resolution(false)
    }

    pub(super) fn render_resize_frame(&mut self) -> Result<(), HtmlBrowserError> {
        self.render_frame_with_resize_resolution(true)
    }

    fn render_frame_with_resize_resolution(
        &mut self,
        resolve_resize_scroll: bool,
    ) -> Result<(), HtmlBrowserError> {
        #[cfg(all(test, target_os = "macos"))]
        let _font_observer =
            crate::markdown::svg_rasterize::SvgRasterizeOps::font_work_frame_scope();
        let generation = self.generation + 1;
        let total_started = self.trace.start();
        let (render, pixels) = SvgRasterizeOps::with_html_font_face_cache_scope(|| {
            self.render_frame_layout_and_rasterize(generation, resolve_resize_scroll)
        })?;
        let started = self.trace.start();
        let result = self.store_frame(render, pixels);
        self.trace.finish(generation, "frame_store", started, &[]);
        self.trace
            .finish(generation, "frame_total", total_started, &[]);
        result
    }

    fn render_frame_layout_and_rasterize(
        &mut self,
        generation: u64,
        resolve_resize_scroll: bool,
    ) -> Result<(LayoutResult, Vec<u8>), HtmlBrowserError> {
        let render = self.layout_frame(generation, resolve_resize_scroll)?;
        let pixels = self.traced_rasterize(
            generation,
            &render.svg,
            &render.generated_clip_provenance,
            &render.generated_html_text_provenance,
        )?;
        Ok((render, pixels))
    }

    fn layout_frame(
        &mut self,
        generation: u64,
        resolve_resize_scroll: bool,
    ) -> Result<LayoutResult, HtmlBrowserError> {
        let _style_declaration_scope = super::style_declaration_cache::Scope::enter();
        let mut render = if resolve_resize_scroll {
            self.layout_resize_frame(generation)?
        } else {
            self.traced_layout(generation)?
        };
        for _ in 0..2 {
            if self.update_scroll(&render) {
                drop(render);
                render = self.traced_layout(generation)?;
            }
            if !self.sync_intersection_geometry(&render)? {
                break;
            }
            drop(render);
            render = self.traced_layout(generation)?;
        }
        Ok(render)
    }

    fn layout_resize_frame(&mut self, generation: u64) -> Result<LayoutResult, HtmlBrowserError> {
        let layout = self.traced_layout(generation)?;
        let previous_scroll = self.scroll_geometry.scroll_y;
        let anchor_scroll = self
            .resize_anchor
            .as_ref()
            .and_then(|anchor| layout.anchor_positions.get(anchor))
            .copied()
            .unwrap_or(previous_scroll);
        self.scroll_geometry.scroll_y =
            anchor_scroll.clamp(0.0, max_scroll_for(&layout, self.viewport));
        if self.scroll_geometry.scroll_y == previous_scroll {
            return Ok(layout);
        }
        drop(layout);
        self.traced_layout(generation)
    }

    pub(super) fn layout(&mut self) -> Result<LayoutResult, HtmlBrowserError> {
        self.traced_layout(self.generation + 1)
    }

    pub(super) fn validate_raster_dimensions(
        &self,
        width: u32,
        height: u32,
    ) -> Result<(), HtmlBrowserError> {
        if width == self.viewport.width && height == self.viewport.height {
            return Ok(());
        }
        Err(runtime_failure(format!(
            "interactive raster dimensions are {width}x{height}, expected {}x{}",
            self.viewport.width, self.viewport.height
        )))
    }

    pub(super) fn store_frame(
        &mut self,
        render: LayoutResult,
        pixels: Vec<u8>,
    ) -> Result<(), HtmlBrowserError> {
        let generation = self.generation + 1;
        let metrics = HtmlBrowserScrollMetrics::new(
            generation,
            self.viewport,
            (render.content_width, render.content_height),
            (self.scroll_geometry.scroll_x, self.scroll_geometry.scroll_y),
            render.viewport_overflow,
        )?;
        let frame = self.raster_frame(pixels, render.content_height)?;
        self.generation = generation;
        self.latest_frame = Some(frame);
        self.latest_scroll_metrics = Some(metrics);
        #[cfg(all(test, target_os = "macos"))]
        crate::markdown::svg_rasterize::SvgRasterizeOps::observe_font_frame(
            self.latest_frame.as_ref(),
            &self.source,
        );
        self.store_layout_geometry(render);
        Ok(())
    }

    fn raster_frame(
        &self,
        pixels: Vec<u8>,
        content_height: f32,
    ) -> Result<HtmlBrowserFrame, HtmlBrowserError> {
        HtmlBrowserFrame::new(
            self.generation + 1,
            self.source.origin.clone(),
            self.viewport,
            HtmlBrowserPixelFormat::Rgba8,
            pixels,
        )
        .map_err(|error| runtime_failure(error.to_string()))
        .map(|frame| frame.with_layout_metrics(self.scroll_geometry.scroll_y, content_height))
    }
}
