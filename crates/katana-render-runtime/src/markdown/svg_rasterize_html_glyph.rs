use resvg::usvg;
use skia_safe::FontMgr;
use std::cell::{OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use types::{CachedFace, CachedTypeface, FaceMetadata};

#[cfg(test)]
use super::composite::TextRasterizer;
#[cfg(test)]
use super::physical_font_face as ttc;

const RGBA_CHANNELS: usize = 4;
const RED_CHANNEL: usize = 0;
const GREEN_CHANNEL: usize = 1;
const BLUE_CHANNEL: usize = 2;
const ALPHA_CHANNEL: usize = 3;

thread_local! {
    /* WHY: 選択済みの物理データの復号だけに使い、frameごとのsystem font列挙を避ける。 */
    static HTML_FONT_MANAGER: FontMgr = FontMgr::default();
}

#[path = "svg_rasterize_html_glyph_alignment.rs"]
mod alignment;
#[path = "svg_rasterize_html_glyph_anchor.rs"]
mod anchor;
pub(super) use alignment::alignment_positioning_supported_with_dy;
#[cfg(test)]
use alignment::alignment_text_supported;
use alignment::{
    alignment_dy_supported, alignment_positioning_supported, alignment_source_span_supported,
    all_zero, horizontal_glyphs, single_horizontal_alignment,
};
#[path = "svg_rasterize_html_glyph_baseline.rs"]
mod baseline;
#[path = "svg_rasterize_html_glyph_chunk.rs"]
mod chunk;
#[path = "svg_rasterize_html_glyph_decorations.rs"]
mod decorations;
#[path = "svg_rasterize_html_glyph_fallback_runs.rs"]
mod fallback_runs;
#[path = "svg_rasterize_html_glyph_multi_chunk_alignment.rs"]
mod multi_chunk_alignment;
#[path = "svg_rasterize_html_glyph_paint.rs"]
mod paint;
#[path = "svg_rasterize_html_glyph_prepare.rs"]
mod prepare;
#[path = "svg_rasterize_html_glyph_runs.rs"]
mod runs;
use types::SpanInput;
#[path = "svg_rasterize_html_glyph_render.rs"]
mod render;
#[cfg(test)]
use render::{uniform_raster_scale, wrap_pixmap};
#[path = "svg_rasterize_html_glyph_face.rs"]
mod face;
#[path = "svg_rasterize_html_glyph_instances.rs"]
mod instances;
#[path = "svg_rasterize_html_glyph_surface.rs"]
mod surface;
#[path = "svg_rasterize_html_glyph_types.rs"]
mod types;

#[cfg(test)]
pub(super) use paint::PaintCaptureScope;

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_decorations_tests.rs"]
mod decorations_tests;

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_multi_chunk_alignment_tests.rs"]
mod multi_chunk_alignment_tests;

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_ttc_integration_tests.rs"]
mod ttc_integration_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_glyph_typeface_tests.rs"]
mod typeface_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_glyph_composite_tests.rs"]
mod composite_tests;

pub(crate) struct HtmlGlyphRasterizer<'a> {
    database: &'a usvg::fontdb::Database,
    faces: RefCell<HashMap<usvg::fontdb::ID, Option<CachedFace>>>,
    variable_faces: RefCell<instances::Cache>,
    font_mgr: OnceCell<FontMgr>,
    resvg_text_ids: HashSet<String>,
    authored_text_context: Option<&'a super::provenance::AuthoredTextContext>,
}

pub(super) fn has_nested_middle_candidate(tree: &usvg::Tree) -> bool {
    baseline::has_nested_middle_candidate(tree)
}

enum Prepared {
    Unsupported,
    Ready(Vec<PreparedSpan>),
}

impl<'a> HtmlGlyphRasterizer<'a> {
    pub(crate) fn new(database: &'a usvg::fontdb::Database) -> Self {
        Self {
            database,
            faces: RefCell::new(HashMap::new()),
            variable_faces: RefCell::new(instances::Cache::default()),
            font_mgr: OnceCell::new(),
            resvg_text_ids: HashSet::new(),
            authored_text_context: None,
        }
    }

    pub(crate) fn with_resvg_text(
        database: &'a usvg::fontdb::Database,
        resvg_text_ids: HashSet<String>,
    ) -> Self {
        Self {
            resvg_text_ids,
            ..Self::new(database)
        }
    }

    pub(super) fn with_resvg_text_and_source_context(
        database: &'a usvg::fontdb::Database,
        resvg_text_ids: HashSet<String>,
        authored_text_context: &'a super::provenance::AuthoredTextContext,
    ) -> Self {
        Self {
            resvg_text_ids,
            authored_text_context: Some(authored_text_context),
            ..Self::new(database)
        }
    }

    pub(super) fn has_authored_nested_text_context(&self, text: &usvg::Text) -> bool {
        self.authored_text_context
            .is_some_and(|context| context.matches(text))
    }

    pub(super) fn has_authored_additional_text_context(&self, text: &usvg::Text) -> bool {
        self.authored_text_context
            .is_some_and(|context| context.matches_additional(text))
    }
}

#[derive(Clone, Copy)]
struct XAlignment {
    anchor_x: f32,
    layout: Option<anchor::LayoutAnchor>,
    origin_snap_css_scale: Option<f32>,
}

struct PreparedSpan {
    fill: [u8; RGBA_CHANNELS],
    font: usvg::fontdb::ID,
    glyphs: Vec<usvg::layout::PositionedGlyph>,
    glyph_ids: Vec<u16>,
    upem: f32,
    font_size: f32,
    baseline_shift: f32,
    x_alignment: Option<XAlignment>,
    chunk_alignment: Option<chunk::SpanAlignment>,
    fallback_alignment: Option<fallback_runs::RunLayout>,
    effects: Option<Box<decorations::Effects>>,
    variable_instance: Option<std::rc::Rc<instances::VariableInstance>>,
}
