// Copyright 2022 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU16;
use std::sync::Arc;

use fontdb::{Database, ID};
use kurbo::{ParamCurve, ParamCurveArclen, ParamCurveDeriv};
use skrifa::MetadataProvider;
use skrifa::Tag;
use skrifa::prelude::Size;
use skrifa::raw::TableProvider;
use strict_num::NonZeroPositiveF32;
use svg_harfrust::ShapeOptions;
use tiny_skia_path::{NonZeroRect, Transform};
use unicode_script::UnicodeScript;

use crate::svg_engine::usvg::tree::{BBox, IsValidLength};
use crate::svg_engine::usvg::{
    AlignmentBaseline, ApproxZeroUlps, BaselineShift, DominantBaseline, Fill, FillRule, Font,
    FontOpticalSizing, FontResolver, GlyphId, LengthAdjust, PaintOrder, Path, ShapeRendering,
    Stroke, Text, TextAnchor, TextChunk, TextDecorationStyle, TextFlow, TextPath, TextSpan,
    WritingMode,
};

/// A glyph that has already been positioned correctly.
///
/// Note that the transform already takes the font size into consideration, so applying the
/// transform to the outline of the glyphs is all that is necessary to display it correctly.
#[derive(Clone, Debug)]
pub struct PositionedGlyph {
    /// Returns the transform of the glyph itself within the cluster. For example,
    /// for zalgo text, it contains the transform to position the glyphs above/below
    /// the main glyph.
    glyph_ts: Transform,
    /// Returns the transform of the whole cluster that the glyph is part of.
    cluster_ts: Transform,
    /// Returns the transform of the span that the glyph is a part of.
    span_ts: Transform,
    /// The units per em of the font the glyph belongs to.
    units_per_em: u16,
    /// The font size the glyph should be scaled to.
    font_size: f32,
    /// The ID of the glyph.
    pub id: GlyphId,
    /// The text from the original string that corresponds to that glyph.
    pub text: String,
    /// The ID of the font the glyph should be taken from. Can be used with the
    /// [font database of the tree](crate::svg_engine::usvg::Tree::fontdb) this glyph is part of.
    pub font: ID,
}

impl PositionedGlyph {
    /// Returns the font size for this glyph.
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Returns the transform of glyph.
    pub fn transform(&self) -> Transform {
        let sx = self.font_size / self.units_per_em as f32;

        self.span_ts
            .pre_concat(self.cluster_ts)
            .pre_concat(Transform::from_scale(sx, sx))
            .pre_concat(self.glyph_ts)
    }

    /// Returns the transform of glyph, assuming that an outline
    /// glyph is being used (i.e. from the `glyf` or `CFF/CFF2` table).
    pub fn outline_transform(&self) -> Transform {
        // Outlines are mirrored by default.
        self.transform()
            .pre_concat(Transform::from_scale(1.0, -1.0))
    }

    /// Returns the transform for the glyph, assuming that a CBTD-based raster glyph
    /// is being used.
    pub fn cbdt_transform(&self, x: f32, y: f32, pixels_per_em: f32) -> Transform {
        self.transform()
            .pre_concat(Transform::from_scale(
                self.units_per_em as f32 / pixels_per_em,
                self.units_per_em as f32 / pixels_per_em,
            ))
            // Right now, the top-left corner of the image would be placed in
            // on the "text cursor", but we want the bottom-left corner to be there,
            // so we need to shift it up and also apply the x/y offset.
            .pre_translate(x, -y)
    }

    /// Returns the transform for the glyph, assuming that a sbix-based raster glyph
    /// is being used.
    pub fn sbix_transform(
        &self,
        x: f32,
        y: f32,
        x_min: f32,
        y_min: f32,
        pixels_per_em: f32,
        height: f32,
    ) -> Transform {
        // In contrast to CBDT, we also need to look at the outline bbox of the glyph and add a shift if necessary.
        let bbox_x_shift = -x_min;

        let bbox_y_shift = if y_min.approx_zero_ulps(4) {
            // For unknown reasons, using Apple Color Emoji will lead to a vertical shift on MacOS, but this shift
            // doesn't seem to be coming from the font and most likely is somehow hardcoded. On Windows,
            // this shift will not be applied. However, if this shift is not applied the emojis are a bit
            // too high up when being together with other text, so we try to imitate this.
            // See also https://github.com/harfbuzz/harfbuzz/issues/2679#issuecomment-1345595425
            // So whenever the y-shift is 0, we approximate this vertical shift that seems to be produced by it.
            // This value seems to be pretty close to what is happening on MacOS.
            // We can still remove this if it turns out to be a problem, but Apple Color Emoji is pretty
            // much the only `sbix` font out there and they all seem to have a y-shift of 0, so it
            // makes sense to keep it.
            0.128 * self.units_per_em as f32
        } else {
            -y_min
        };

        self.transform()
            .pre_concat(Transform::from_translate(bbox_x_shift, bbox_y_shift))
            .pre_concat(Transform::from_scale(
                self.units_per_em as f32 / pixels_per_em,
                self.units_per_em as f32 / pixels_per_em,
            ))
            // Right now, the top-left corner of the image would be placed in
            // on the "text cursor", but we want the bottom-left corner to be there,
            // so we need to shift it up and also apply the x/y offset.
            .pre_translate(x, -height - y)
    }

    /// Returns the transform for the glyph, assuming that an SVG glyph is
    /// being used.
    pub fn svg_transform(&self) -> Transform {
        self.transform()
    }

    /// Returns the transform for the glyph, assuming that a COLR glyph is
    /// being used.
    pub fn colr_transform(&self) -> Transform {
        self.outline_transform()
    }
}

/// A span contains a number of layouted glyphs that share the same fill, stroke, paint order and
/// visibility.
#[derive(Clone, Debug)]
pub struct Span {
    /// The fill of the span.
    pub fill: Option<Fill>,
    /// The stroke of the span.
    pub stroke: Option<Stroke>,
    /// The paint order of the span.
    pub paint_order: PaintOrder,
    /// The font size of the span.
    pub font_size: NonZeroPositiveF32,
    /// Font variation settings for variable fonts.
    pub variations: Vec<crate::svg_engine::usvg::FontVariation>,
    /// Font optical sizing mode.
    pub font_optical_sizing: crate::svg_engine::usvg::FontOpticalSizing,
    /// The visibility of the span.
    pub visible: bool,
    /// The glyphs that make up the span.
    pub positioned_glyphs: Vec<PositionedGlyph>,
    /// An underline text decoration of the span.
    /// Needs to be rendered before all glyphs.
    pub underline: Option<Path>,
    /// An overline text decoration of the span.
    /// Needs to be rendered before all glyphs.
    pub overline: Option<Path>,
    /// A line-through text decoration of the span.
    /// Needs to be rendered after all glyphs.
    pub line_through: Option<Path>,
}

#[derive(Clone, Debug)]
struct GlyphCluster {
    byte_idx: ByteIndex,
    codepoint: char,
    width: f32,
    advance: f32,
    text_length_spacing: f32,
    ascent: f32,
    descent: f32,
    has_relative_shift: bool,
    glyphs: Vec<PositionedGlyph>,
    transform: Transform,
    path_transform: Transform,
    visible: bool,
}

impl GlyphCluster {
    pub(crate) fn height(&self) -> f32 {
        self.ascent - self.descent
    }

    pub(crate) fn transform(&self) -> Transform {
        self.path_transform.post_concat(self.transform)
    }
}

pub(crate) fn layout_text(
    text_node: &Text,
    resolver: &FontResolver,
    fontdb: &mut Arc<fontdb::Database>,
) -> Option<(Vec<Span>, NonZeroRect)> {
    let fonts_cache = load_text_fonts(text_node, resolver, fontdb);
    let mut state = TextLayoutState::default();
    for chunk in &text_node.chunks {
        layout_chunk(text_node, chunk, &fonts_cache, resolver, fontdb, &mut state);
    }
    let bbox = state.bbox.to_non_zero_rect()?;
    Some((state.spans, bbox))
}

#[derive(Default)]
struct TextLayoutState {
    spans: Vec<Span>,
    char_offset: usize,
    last_x: f32,
    last_y: f32,
    bbox: BBox,
}

fn layout_chunk(
    text_node: &Text,
    chunk: &TextChunk,
    fonts_cache: &FontsCache,
    resolver: &FontResolver,
    fontdb: &mut Arc<fontdb::Database>,
    state: &mut TextLayoutState,
) {
    let (x, y) = chunk_origin(chunk, state);
    let prepared = prepare_layout_clusters(
        text_node,
        chunk,
        fonts_cache,
        resolver,
        fontdb,
        state.char_offset,
    );
    if let Some((clusters, mut curr_pos)) = prepared {
        let text_ts = chunk_text_transform(text_node.writing_mode, &chunk.text_flow, x, y);
        let context = SpanLayoutContext {
            text_node,
            chunk,
            fonts_cache,
            clusters: &clusters,
            text_ts,
            x,
            y,
        };
        layout_chunk_spans(&context, &mut state.bbox, &mut state.spans);
        state.char_offset += chunk.text.chars().count();
        finish_chunk_position(state, &context, &mut curr_pos);
    } else {
        state.char_offset += chunk.text.chars().count();
    }
}

fn chunk_origin(chunk: &TextChunk, state: &TextLayoutState) -> (f32, f32) {
    match chunk.text_flow {
        TextFlow::Linear => (
            chunk.x.unwrap_or(state.last_x),
            chunk.y.unwrap_or(state.last_y),
        ),
        TextFlow::Path(_) => (0.0, 0.0),
    }
}

fn finish_chunk_position(
    state: &mut TextLayoutState,
    context: &SpanLayoutContext<'_>,
    curr_pos: &mut (f32, f32),
) {
    if context.text_node.writing_mode == WritingMode::TopToBottom {
        if let TextFlow::Linear = context.chunk.text_flow {
            std::mem::swap(&mut curr_pos.0, &mut curr_pos.1);
        }
    }
    state.last_x = context.x + curr_pos.0;
    state.last_y = context.y + curr_pos.1;
}

fn prepare_layout_clusters(
    text_node: &Text,
    chunk: &TextChunk,
    fonts_cache: &FontsCache,
    resolver: &FontResolver,
    fontdb: &mut Arc<fontdb::Database>,
    char_offset: usize,
) -> Option<(Vec<GlyphCluster>, (f32, f32))> {
    let mut clusters = process_chunk(chunk, fonts_cache, resolver, fontdb, text_node.writing_mode);
    if clusters.is_empty() {
        return None;
    }
    apply_length_adjust(chunk, &mut clusters);
    let curr_pos = resolve_clusters_positions(
        text_node,
        chunk,
        char_offset,
        text_node.writing_mode,
        fonts_cache,
        &mut clusters,
    );
    Some((clusters, curr_pos))
}

fn chunk_text_transform(mode: WritingMode, flow: &TextFlow, x: f32, y: f32) -> Transform {
    let mut transform = Transform::default();
    if mode == WritingMode::TopToBottom {
        if let TextFlow::Linear = flow {
            transform = transform.pre_rotate_at(90.0, x, y);
        }
    }
    transform
}

struct SpanLayoutContext<'a> {
    text_node: &'a Text,
    chunk: &'a TextChunk,
    fonts_cache: &'a FontsCache,
    clusters: &'a [GlyphCluster],
    text_ts: Transform,
    x: f32,
    y: f32,
}

fn layout_chunk_spans(context: &SpanLayoutContext<'_>, bbox: &mut BBox, spans: &mut Vec<Span>) {
    for span in &context.chunk.spans {
        layout_one_span(context, span, bbox, spans);
    }
}

fn layout_one_span(
    context: &SpanLayoutContext<'_>,
    span: &TextSpan,
    bbox: &mut BBox,
    spans: &mut Vec<Span>,
) {
    let Some(font) = context.fonts_cache.get(&span.font) else {
        return;
    };
    let decoration_spans = collect_decoration_spans(span, context.clusters);
    let mut span_ts = context.text_ts;
    span_ts = span_ts.pre_translate(context.x, context.y);
    if let TextFlow::Linear = context.chunk.text_flow {
        let shift = resolve_baseline(span, font, context.text_node.writing_mode);
        span_ts = span_ts.pre_translate(0.0, shift);
    }
    let decorations = layout_span_decorations(
        span,
        font,
        context.text_node.writing_mode,
        &decoration_spans,
        span_ts,
        bbox,
    );
    let mut fill = span.fill.clone();
    if let Some(ref mut fill) = fill {
        fill.rule = FillRule::NonZero;
    }
    if let Some((fragments, span_bbox)) = convert_span(span, context.clusters, span_ts) {
        *bbox = bbox.expand(span_bbox);
        let positioned_glyphs = position_span_glyphs(fragments, span_ts);
        spans.push(make_layout_span(span, fill, positioned_glyphs, decorations));
    }
}

struct SpanDecorations {
    underline: Option<Path>,
    overline: Option<Path>,
    line_through: Option<Path>,
}

struct DecorationContext<'a> {
    span: &'a TextSpan,
    font: &'a ResolvedFont,
    mode: WritingMode,
    spans: &'a [DecorationSpan],
    ts: Transform,
}

fn layout_span_decorations(
    span: &TextSpan,
    font: &ResolvedFont,
    mode: WritingMode,
    spans: &[DecorationSpan],
    ts: Transform,
    bbox: &mut BBox,
) -> SpanDecorations {
    let context = DecorationContext {
        span,
        font,
        mode,
        spans,
        ts,
    };
    let underline = layout_underline(&context, bbox);
    let overline = layout_overline(&context, bbox);
    let line_through = layout_line_through(&context, bbox);
    SpanDecorations {
        underline,
        overline,
        line_through,
    }
}

fn layout_underline(context: &DecorationContext<'_>, bbox: &mut BBox) -> Option<Path> {
    let decoration = context.span.decoration.underline.clone()?;
    let offset = match context.mode {
        WritingMode::LeftToRight => -context
            .font
            .underline_position(context.span.font_size.get()),
        WritingMode::TopToBottom => context.font.height(context.span.font_size.get()) / 2.0,
    };
    decoration_with_bounds(context, decoration, offset, bbox)
}

fn layout_overline(context: &DecorationContext<'_>, bbox: &mut BBox) -> Option<Path> {
    let decoration = context.span.decoration.overline.clone()?;
    let offset = match context.mode {
        WritingMode::LeftToRight => -context.font.ascent(context.span.font_size.get()),
        WritingMode::TopToBottom => -context.font.height(context.span.font_size.get()) / 2.0,
    };
    decoration_with_bounds(context, decoration, offset, bbox)
}

fn layout_line_through(context: &DecorationContext<'_>, bbox: &mut BBox) -> Option<Path> {
    let decoration = context.span.decoration.line_through.clone()?;
    let offset = match context.mode {
        WritingMode::LeftToRight => -context
            .font
            .line_through_position(context.span.font_size.get()),
        WritingMode::TopToBottom => 0.0,
    };
    decoration_with_bounds(context, decoration, offset, bbox)
}

fn decoration_with_bounds(
    context: &DecorationContext<'_>,
    decoration: TextDecorationStyle,
    offset: f32,
    bbox: &mut BBox,
) -> Option<Path> {
    let path = convert_decoration(
        offset,
        context.span,
        context.font,
        decoration,
        context.spans,
        context.ts,
    )?;
    *bbox = bbox.expand(path.data.bounds());
    Some(path)
}

fn position_span_glyphs(fragments: Vec<GlyphCluster>, span_ts: Transform) -> Vec<PositionedGlyph> {
    fragments
        .into_iter()
        .flat_map(|mut gc| {
            let cluster_ts = gc.transform();
            gc.glyphs.iter_mut().for_each(|pg| {
                pg.cluster_ts = cluster_ts;
                pg.span_ts = span_ts;
            });
            gc.glyphs
        })
        .collect()
}

fn make_layout_span(
    span: &TextSpan,
    fill: Option<Fill>,
    positioned_glyphs: Vec<PositionedGlyph>,
    decorations: SpanDecorations,
) -> Span {
    Span {
        fill,
        stroke: span.stroke.clone(),
        paint_order: span.paint_order,
        font_size: span.font_size,
        variations: span.font.variations.clone(),
        font_optical_sizing: span.font_optical_sizing,
        visible: span.visible,
        positioned_glyphs,
        underline: decorations.underline,
        overline: decorations.overline,
        line_through: decorations.line_through,
    }
}

fn load_text_fonts(
    text_node: &Text,
    resolver: &FontResolver,
    fontdb: &mut Arc<fontdb::Database>,
) -> FontsCache {
    let mut fonts_cache = HashMap::new();
    for chunk in &text_node.chunks {
        for span in &chunk.spans {
            if !fonts_cache.contains_key(&span.font) {
                if let Some(font) = (resolver.select_font)(&span.font, fontdb)
                    .and_then(|id| fontdb.load_font(id, &span.font.variations))
                {
                    fonts_cache.insert(span.font.clone(), Arc::new(font));
                }
            }
        }
    }
    fonts_cache
}

fn convert_span(
    span: &TextSpan,
    clusters: &[GlyphCluster],
    text_ts: Transform,
) -> Option<(Vec<GlyphCluster>, NonZeroRect)> {
    let mut span_clusters = vec![];
    let mut bboxes_builder = tiny_skia_path::PathBuilder::new();

    for cluster in clusters {
        if !cluster.visible {
            continue;
        }

        if span_contains(span, cluster.byte_idx) {
            span_clusters.push(cluster.clone());
        }

        push_cluster_bbox(cluster, &mut bboxes_builder);
    }

    let bboxes = bboxes_builder.finish()?.transform(text_ts)?;
    // 矩形の直線だけを追加するため、変換済みの点の境界がそのままタイト境界になる。
    let bbox = bboxes.bounds().to_non_zero_rect()?;

    Some((span_clusters, bbox))
}

fn push_cluster_bbox(cluster: &GlyphCluster, builder: &mut tiny_skia_path::PathBuilder) {
    let mut advance = cluster.advance;
    if advance <= 0.0 {
        advance = 1.0;
    }
    // We have to calculate text bbox using font metrics and not glyph shape.
    if let Some(r) = NonZeroRect::from_xywh(0.0, -cluster.ascent, advance, cluster.height()) {
        if let Some(r) = r.transform(cluster.transform()) {
            builder.push_rect(r.to_rect());
        }
    }
}

fn collect_decoration_spans(span: &TextSpan, clusters: &[GlyphCluster]) -> Vec<DecorationSpan> {
    let mut spans = Vec::new();

    let mut started = false;
    let mut width = 0.0;
    let mut transform = Transform::default();

    for cluster in clusters {
        if span_contains(span, cluster.byte_idx) {
            if started && cluster.has_relative_shift {
                finish_decoration_span(&mut spans, &mut started, width, transform);
            }

            if !started {
                width = cluster.advance;
                started = true;
                transform = cluster.transform;
            } else {
                width += cluster.advance;
            }
        } else {
            finish_decoration_span(&mut spans, &mut started, width, transform);
        }
    }

    finish_decoration_span(&mut spans, &mut started, width, transform);
    spans
}

fn finish_decoration_span(
    spans: &mut Vec<DecorationSpan>,
    started: &mut bool,
    width: f32,
    transform: Transform,
) {
    if *started {
        spans.push(DecorationSpan { width, transform });
        *started = false;
    }
}

pub(crate) fn convert_decoration(
    dy: f32,
    span: &TextSpan,
    font: &ResolvedFont,
    mut decoration: TextDecorationStyle,
    decoration_spans: &[DecorationSpan],
    transform: Transform,
) -> Option<Path> {
    debug_assert!(!decoration_spans.is_empty());

    let thickness = font.underline_thickness(span.font_size.get());

    let mut builder = tiny_skia_path::PathBuilder::new();
    for dec_span in decoration_spans {
        if let Some(path) = decoration_span_path(dec_span, thickness, dy) {
            builder.push_path(&path);
        }
    }

    let mut path_data = builder.finish()?;
    path_data = path_data.transform(transform)?;

    Path::new(
        String::new(),
        span.visible,
        decoration.fill.take(),
        decoration.stroke.take(),
        PaintOrder::default(),
        ShapeRendering::default(),
        Arc::new(path_data),
        Transform::default(),
    )
}

fn decoration_span_path(
    span: &DecorationSpan,
    thickness: f32,
    dy: f32,
) -> Option<tiny_skia_path::Path> {
    let rect = match NonZeroRect::from_xywh(0.0, -thickness / 2.0, span.width, thickness) {
        Some(v) => v,
        None => {
            log::warn!("a decoration span has a malformed bbox");
            return None;
        }
    };
    let ts = span.transform.pre_translate(0.0, dy);
    tiny_skia_path::PathBuilder::from_rect(rect.to_rect()).transform(ts)
}

/// A text decoration span.
///
/// Basically a horizontal line, that will be used for underline, overline and line-through.
/// It doesn't have a height, since it depends on the Font metrics.
#[derive(Clone, Copy)]
pub(crate) struct DecorationSpan {
    pub(crate) width: f32,
    pub(crate) transform: Transform,
}

/// Resolves clusters positions.
///
/// Mainly sets the `transform` property.
///
/// Returns the last text position. The next text chunk should start from that position.
fn resolve_clusters_positions(
    text: &Text,
    chunk: &TextChunk,
    char_offset: usize,
    writing_mode: WritingMode,
    fonts_cache: &FontsCache,
    clusters: &mut [GlyphCluster],
) -> (f32, f32) {
    match chunk.text_flow {
        TextFlow::Linear => {
            resolve_clusters_positions_horizontal(text, chunk, char_offset, writing_mode, clusters)
        }
        TextFlow::Path(ref path) => resolve_clusters_positions_path(
            text,
            chunk,
            char_offset,
            path,
            writing_mode,
            fonts_cache,
            clusters,
        ),
    }
}

fn clusters_length(clusters: &[GlyphCluster]) -> f32 {
    clusters.iter().fold(0.0, |w, cluster| w + cluster.advance)
}

fn resolve_relative_shifts(
    text: &Text,
    chunk: &TextChunk,
    offset: usize,
    clusters: &[GlyphCluster],
) -> (Vec<(f32, f32)>, Vec<usize>) {
    // `dx` and `dy` shift a character and all the characters after it in logical order,
    // while the clusters are in visual order, which differs in right-to-left text.
    let mut logical_order: Vec<usize> = (0..clusters.len()).collect();
    logical_order.sort_by_key(|&i| clusters[i].byte_idx.value());

    let mut shifts = vec![(0.0, 0.0); clusters.len()];
    let (mut shift_x, mut shift_y) = (0.0, 0.0);
    for &i in &logical_order {
        let cp = offset + clusters[i].byte_idx.code_point_at(&chunk.text);
        shift_x += text.dx.get(cp).cloned().unwrap_or(0.0);
        shift_y += text.dy.get(cp).cloned().unwrap_or(0.0);
        shifts[i] = (shift_x, shift_y);
    }

    (shifts, logical_order)
}

fn resolve_clusters_positions_horizontal(
    text: &Text,
    chunk: &TextChunk,
    offset: usize,
    writing_mode: WritingMode,
    clusters: &mut [GlyphCluster],
) -> (f32, f32) {
    let (mut shifts, logical_order) = resolve_relative_shifts(text, chunk, offset, clusters);
    let first = logical_order[0];
    let last = logical_order[logical_order.len() - 1];

    // Vertical text is laid out horizontally and rotated by 90 degrees afterwards.
    if writing_mode == WritingMode::TopToBottom {
        for shift in &mut shifts {
            *shift = (shift.1, -shift.0);
        }
    }

    for i in 1..clusters.len() {
        if shifts[i] != shifts[i - 1] {
            clusters[i].has_relative_shift = true;
        }
    }

    let (left, right) = horizontal_cluster_extent(clusters, &shifts);
    let x = shifts[first].0 - left + process_anchor(chunk.anchor, right - left);
    let x = place_horizontal_clusters(text, chunk, offset, clusters, &shifts, x);

    (x + shifts[last].0, shifts[last].1)
}

fn place_horizontal_clusters(
    text: &Text,
    chunk: &TextChunk,
    offset: usize,
    clusters: &mut [GlyphCluster],
    shifts: &[(f32, f32)],
    mut x: f32,
) -> f32 {
    for (cluster, (shift_x, shift_y)) in clusters.iter_mut().zip(shifts) {
        let cp = offset + cluster.byte_idx.code_point_at(&chunk.text);

        cluster.transform = cluster.transform.pre_translate(x + shift_x, *shift_y);

        let angle = text.rotate.get(cp).copied().unwrap_or_default();
        if !angle.approx_zero_ulps(4) {
            cluster.transform = cluster.transform.pre_rotate(angle);
            cluster.has_relative_shift = true;
        }

        x += cluster.advance;
    }

    x
}

fn horizontal_cluster_extent(clusters: &[GlyphCluster], shifts: &[(f32, f32)]) -> (f32, f32) {
    let mut left = f32::INFINITY;
    let mut right = f32::NEG_INFINITY;
    let mut advance = 0.0;
    for (cluster, (shift_x, _)) in clusters.iter().zip(shifts) {
        let start = advance + shift_x;
        let end = start + cluster.advance - cluster.text_length_spacing;
        left = left.min(start).min(end);
        right = right.max(start).max(end);
        advance += cluster.advance;
    }
    (left, right)
}

// Baseline resolving in SVG is a mess.
// Not only it's poorly documented, but as soon as you start mixing
// `dominant-baseline` and `alignment-baseline` each application/browser will produce
// different results.
//
// For now, resvg simply tries to match Chrome's output and not the mythical SVG spec output.
//
// See `alignment_baseline_shift` method comment for more details.
pub(crate) fn resolve_baseline(
    span: &TextSpan,
    font: &ResolvedFont,
    writing_mode: WritingMode,
) -> f32 {
    let mut shift = -resolve_baseline_shift(&span.baseline_shift, font, span.font_size.get());

    // TODO: support vertical layout as well
    if writing_mode == WritingMode::LeftToRight {
        if span.alignment_baseline == AlignmentBaseline::Auto
            || span.alignment_baseline == AlignmentBaseline::Baseline
        {
            shift += font.dominant_baseline_shift(span.dominant_baseline, span.font_size.get());
        } else {
            shift += font.alignment_baseline_shift(span.alignment_baseline, span.font_size.get());
        }
    }

    shift
}

fn resolve_baseline_shift(baselines: &[BaselineShift], font: &ResolvedFont, font_size: f32) -> f32 {
    let mut shift = 0.0;
    for baseline in baselines.iter().rev() {
        match baseline {
            BaselineShift::Baseline => {}
            BaselineShift::Subscript => shift -= font.subscript_offset(font_size),
            BaselineShift::Superscript => shift += font.superscript_offset(font_size),
            BaselineShift::Number(n) => shift += n,
        }
    }

    shift
}

fn resolve_clusters_positions_path(
    text: &Text,
    chunk: &TextChunk,
    char_offset: usize,
    path: &TextPath,
    writing_mode: WritingMode,
    fonts_cache: &FontsCache,
    clusters: &mut [GlyphCluster],
) -> (f32, f32) {
    let mut last_x = 0.0;
    let mut last_y = 0.0;

    let (shifts, _) = resolve_relative_shifts(text, chunk, char_offset, clusters);

    let start_offset = text_path_start_offset(chunk, path, writing_mode, clusters);

    let normals = collect_normals(text, clusters, &shifts, &path.path, start_offset);
    let placement = PathPlacement {
        text,
        chunk,
        char_offset,
        writing_mode,
        fonts_cache,
    };
    for ((cluster, normal), (_, dy)) in clusters.iter_mut().zip(normals).zip(shifts) {
        if let Some((x, y)) = position_cluster_on_path(&placement, cluster, normal, dy) {
            last_x = x;
            last_y = y;
        }
    }

    (last_x, last_y)
}

fn text_path_start_offset(
    chunk: &TextChunk,
    path: &TextPath,
    writing_mode: WritingMode,
    clusters: &[GlyphCluster],
) -> f32 {
    // textPath上の追加offsetには横書きでx、縦書きでyを使う。
    let chunk_offset = match writing_mode {
        WritingMode::LeftToRight => chunk.x.unwrap_or(0.0),
        WritingMode::TopToBottom => chunk.y.unwrap_or(0.0),
    };
    chunk_offset + path.start_offset + process_anchor(chunk.anchor, clusters_length(clusters))
}

struct PathPlacement<'a> {
    text: &'a Text,
    chunk: &'a TextChunk,
    char_offset: usize,
    writing_mode: WritingMode,
    fonts_cache: &'a FontsCache,
}

fn position_cluster_on_path(
    placement: &PathPlacement<'_>,
    cluster: &mut GlyphCluster,
    normal: Option<PathNormal>,
    dy: f32,
) -> Option<(f32, f32)> {
    let Some(normal) = normal else {
        cluster.visible = false;
        return None;
    };
    let (x, y, angle) = (normal.x, normal.y, normal.angle);
    cluster.has_relative_shift = true;
    let orig_ts = cluster.transform;
    let half_width = cluster.width / 2.0;
    cluster.transform = Transform::default();
    cluster.transform = cluster.transform.pre_translate(x - half_width, y);
    cluster.transform = cluster.transform.pre_rotate_at(angle, half_width, 0.0);
    let cp = placement.char_offset + cluster.byte_idx.code_point_at(&placement.chunk.text);
    let baseline_shift = path_baseline_shift(placement, cluster);
    apply_path_offsets(placement, cluster, cp, dy, baseline_shift);
    cluster.transform = cluster.transform.pre_concat(orig_ts);
    Some((x + cluster.advance, y))
}

fn apply_path_offsets(
    placement: &PathPlacement<'_>,
    cluster: &mut GlyphCluster,
    cp: usize,
    dy: f32,
    baseline_shift: f32,
) {
    if !dy.approx_zero_ulps(4) || !baseline_shift.approx_zero_ulps(4) {
        let shift = kurbo::Vec2::new(0.0, (dy - baseline_shift) as f64);
        cluster.transform = cluster
            .transform
            .pre_translate(shift.x as f32, shift.y as f32);
    }
    let angle = placement.text.rotate.get(cp).copied().unwrap_or_default();
    if !angle.approx_zero_ulps(4) {
        cluster.transform = cluster.transform.pre_rotate(angle);
    }
}

fn path_baseline_shift(placement: &PathPlacement<'_>, cluster: &GlyphCluster) -> f32 {
    chunk_span_at(placement.chunk, cluster.byte_idx)
        .map(|span| {
            let font = match placement.fonts_cache.get(&span.font) {
                Some(v) => v,
                None => return 0.0,
            };
            -resolve_baseline(span, font, placement.writing_mode)
        })
        .unwrap_or(0.0)
}

pub(crate) fn process_anchor(a: TextAnchor, text_width: f32) -> f32 {
    match a {
        TextAnchor::Start => 0.0, // Nothing.
        TextAnchor::Middle => -text_width / 2.0,
        TextAnchor::End => -text_width,
    }
}

pub(crate) struct PathNormal {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) angle: f32,
}

fn collect_normals(
    text: &Text,
    clusters: &[GlyphCluster],
    shifts: &[(f32, f32)],
    path: &tiny_skia_path::Path,
    offset: f32,
) -> Vec<Option<PathNormal>> {
    let (offsets, mut normals) = path_cluster_offsets(clusters, shifts, offset);
    let mut cursor = NormalPathCursor::new(path);
    let mut length = 0.0;
    for segment in path.segments() {
        let Some(curve) = cursor.curve(segment) else {
            continue;
        };
        let (sx, sy) = text.abs_transform.get_scale();
        let accuracy = 0.5 / (sx * sy).sqrt().max(1.0);
        let curve_len = curve.arclen(accuracy as f64);
        append_curve_normals(&curve, &offsets, &mut normals, length, curve_len, accuracy);
        length += curve_len;
        cursor.advance(&curve);
    }
    for _ in 0..(offsets.len() - normals.len()) {
        normals.push(None);
    }
    normals
}

fn path_cluster_offsets(
    clusters: &[GlyphCluster],
    shifts: &[(f32, f32)],
    offset: f32,
) -> (Vec<f64>, Vec<Option<PathNormal>>) {
    let mut offsets = Vec::with_capacity(clusters.len());
    let mut normals = Vec::with_capacity(clusters.len());
    let mut advance = offset;
    for (cluster, (shift_x, _)) in clusters.iter().zip(shifts) {
        let half_width = cluster.width / 2.0;
        let offset = advance + shift_x + half_width;
        if offset < 0.0 {
            normals.push(None);
        }
        offsets.push(offset as f64);
        advance += cluster.advance;
    }
    (offsets, normals)
}

struct NormalPathCursor {
    prev_mx: f32,
    prev_my: f32,
    prev_x: f32,
    prev_y: f32,
}

impl NormalPathCursor {
    fn new(path: &tiny_skia_path::Path) -> Self {
        let prev_mx = path.points()[0].x;
        let prev_my = path.points()[0].y;
        Self {
            prev_mx,
            prev_my,
            prev_x: prev_mx,
            prev_y: prev_my,
        }
    }

    fn curve(&mut self, seg: tiny_skia_path::PathSegment) -> Option<kurbo::CubicBez> {
        use tiny_skia_path::PathSegment;
        match seg {
            PathSegment::MoveTo(p) => {
                self.prev_mx = p.x;
                self.prev_my = p.y;
                self.prev_x = p.x;
                self.prev_y = p.y;
                None
            }
            PathSegment::LineTo(p) => {
                Some(normal_curve_from_line(self.prev_x, self.prev_y, p.x, p.y))
            }
            PathSegment::QuadTo(p1, p) => Some(self.quadratic(p1, p)),
            PathSegment::CubicTo(p1, p2, p) => Some(self.cubic(p1, p2, p)),
            PathSegment::Close => Some(normal_curve_from_line(
                self.prev_x,
                self.prev_y,
                self.prev_mx,
                self.prev_my,
            )),
        }
    }

    fn advance(&mut self, curve: &kurbo::CubicBez) {
        self.prev_x = curve.p3.x as f32;
        self.prev_y = curve.p3.y as f32;
    }

    fn quadratic(&self, p1: tiny_skia_path::Point, p: tiny_skia_path::Point) -> kurbo::CubicBez {
        kurbo::QuadBez {
            p0: kurbo::Point::new(self.prev_x as f64, self.prev_y as f64),
            p1: kurbo::Point::new(p1.x as f64, p1.y as f64),
            p2: kurbo::Point::new(p.x as f64, p.y as f64),
        }
        .raise()
    }

    fn cubic(
        &self,
        p1: tiny_skia_path::Point,
        p2: tiny_skia_path::Point,
        p: tiny_skia_path::Point,
    ) -> kurbo::CubicBez {
        kurbo::CubicBez {
            p0: kurbo::Point::new(self.prev_x as f64, self.prev_y as f64),
            p1: kurbo::Point::new(p1.x as f64, p1.y as f64),
            p2: kurbo::Point::new(p2.x as f64, p2.y as f64),
            p3: kurbo::Point::new(p.x as f64, p.y as f64),
        }
    }
}

fn normal_curve_from_line(px: f32, py: f32, x: f32, y: f32) -> kurbo::CubicBez {
    let line = kurbo::Line::new(
        kurbo::Point::new(px as f64, py as f64),
        kurbo::Point::new(x as f64, y as f64),
    );
    let p1 = line.eval(0.33);
    let p2 = line.eval(0.66);
    kurbo::CubicBez {
        p0: line.p0,
        p1,
        p2,
        p3: line.p1,
    }
}

fn append_curve_normals(
    curve: &kurbo::CubicBez,
    offsets: &[f64],
    normals: &mut Vec<Option<PathNormal>>,
    length: f64,
    curve_len: f64,
    accuracy: f32,
) {
    for offset in &offsets[normals.len()..] {
        if *offset >= length && *offset <= length + curve_len {
            let mut offset = curve.inv_arclen(offset - length, accuracy as f64);
            debug_assert!((-1.0e-3..=1.0 + 1.0e-3).contains(&offset));
            offset = offset.clamp(0.0, 1.0);
            let pos = curve.eval(offset);
            let d = curve.deriv().eval(offset);
            let d = kurbo::Vec2::new(-d.y, d.x);
            let angle = d.atan2().to_degrees() - 90.0;
            normals.push(Some(PathNormal {
                x: pos.x as f32,
                y: pos.y as f32,
                angle: angle as f32,
            }));
            if normals.len() == offsets.len() {
                break;
            }
        }
    }
}

// The way this function works is a bit tricky.
//
// The first problem is BIDI reordering.
// We cannot shape text span-by-span, because glyph clusters are not guarantee to be continuous.
//
// For example:
// <text>Hel<tspan fill="url(#lg1)">lo של</tspan>ום.</text>
//
// Would be shaped as:
// H e l l o   ש ל  ו  ם .   (characters)
// 0 1 2 3 4 5 12 10 8 6 14  (cluster indices in UTF-8)
//       ---         ---     (green span)
//
// As you can see, our continuous `lo של` span was split into two separated one.
// So our 3 spans: black - green - black, become 5 spans: black - green - black - green - black.
// If we shape `Hel`, then `lo של` an then `ום` separately - we would get an incorrect output.
// To properly handle this we simply shape the whole chunk.
//
// But this introduces another issue - what to do when we have multiple fonts?
// The easy solution would be to simply shape text with each font,
// where the first font output is used as a base one and all others overwrite it.
// This way in case of:
// <text font-family="Arial">Hello <tspan font-family="Helvetica">world</tspan></text>
// we would replace Arial glyphs for `world` with Helvetica one. Pretty simple.
//
// Well, it would work most of the time, but not always.
// This is because different fonts can produce different amount of glyphs for the same text.
// The most common example are ligatures. Some fonts can shape `fi` as two glyphs `f` and `i`,
// but some can use `ﬁ` (U+FB01) instead.
// Meaning that during merging we have to overwrite not individual glyphs, but clusters.

/// Converts a text chunk into a list of outlined clusters.
///
/// This function will do the BIDI reordering, text shaping and glyphs outlining,
/// but not the text layouting. So all clusters are in the 0x0 position.
fn process_chunk(
    chunk: &TextChunk,
    fonts_cache: &FontsCache,
    resolver: &FontResolver,
    fontdb: &mut Arc<fontdb::Database>,
    writing_mode: WritingMode,
) -> Vec<GlyphCluster> {
    let glyphs = shape_chunk_glyphs(chunk, fonts_cache, resolver, fontdb);
    glyphs_to_clusters(chunk, &glyphs, writing_mode)
}

fn shape_chunk_glyphs(
    chunk: &TextChunk,
    fonts_cache: &FontsCache,
    resolver: &FontResolver,
    fontdb: &mut Arc<fontdb::Database>,
) -> Vec<Glyph> {
    // Glyph splitting assigns distinct glyphs to the same index in the original text, we need to
    // store previously used indices to make sure we do not re-use the same index while overwriting
    // span glyphs.
    let mut positions = HashSet::new();

    let mut glyphs = Vec::new();
    for span in &chunk.spans {
        let font = match fonts_cache.get(&span.font) {
            Some(v) => v.clone(),
            None => continue,
        };

        let tmp_glyphs = shape_chunk_span(chunk, span, font, resolver, fontdb);

        // Do nothing with the first run.
        if glyphs.is_empty() {
            glyphs = tmp_glyphs;
            continue;
        }

        merge_span_glyphs(span, &mut glyphs, tmp_glyphs, &mut positions);
    }

    glyphs
}

fn shape_chunk_span(
    chunk: &TextChunk,
    span: &TextSpan,
    font: Arc<ResolvedFont>,
    resolver: &FontResolver,
    fontdb: &mut Arc<fontdb::Database>,
) -> Vec<Glyph> {
    shape_text(
        &chunk.text,
        font,
        span.small_caps,
        span.apply_kerning,
        &span.font,
        span.font_size.get(),
        span.font_optical_sizing,
        resolver,
        fontdb,
    )
}

fn merge_span_glyphs(
    span: &TextSpan,
    glyphs: &mut Vec<Glyph>,
    new_glyphs: Vec<Glyph>,
    positions: &mut HashSet<usize>,
) {
    positions.clear();
    let mut iter = new_glyphs.into_iter();
    while let Some(new_glyph) = iter.next() {
        if !span_contains(span, new_glyph.byte_idx) {
            continue;
        }

        let Some(idx) = glyphs
            .iter()
            .position(|g| g.byte_idx == new_glyph.byte_idx)
            .filter(|pos| !positions.contains(pos))
        else {
            continue;
        };

        positions.insert(idx);

        resize_glyph_cluster(glyphs, idx, new_glyph.cluster_len, &mut iter);

        glyphs[idx] = new_glyph;
    }
}

fn resize_glyph_cluster(
    glyphs: &mut Vec<Glyph>,
    idx: usize,
    new_len: usize,
    iter: &mut std::vec::IntoIter<Glyph>,
) {
    let prev_cluster_len = glyphs[idx].cluster_len;
    if prev_cluster_len < new_len {
        for _ in 1..new_len {
            glyphs.remove(idx + 1);
        }
    } else if prev_cluster_len > new_len {
        for j in 1..prev_cluster_len {
            if let Some(g) = iter.next() {
                glyphs.insert(idx + j, g);
            }
        }
    }
}

fn glyphs_to_clusters(
    chunk: &TextChunk,
    glyphs: &[Glyph],
    writing_mode: WritingMode,
) -> Vec<GlyphCluster> {
    let has_letter_spacing = chunk.spans.iter().any(|span| !span.letter_spacing.approx_zero_ulps(4));
    let has_word_spacing = chunk.spans.iter().any(|span| !span.word_spacing.approx_zero_ulps(4));
    // 字形クラスタの生成と間隔調整で同じspanを使う。末尾は生成可能なクラスタで判定する。
    let mut selected = GlyphClusters::new(glyphs)
        .filter_map(|(range, offset)| chunk_span_at(chunk, offset).map(|span| (range, span)))
        .peekable();
    let mut clusters = Vec::new();
    while let Some((range, span)) = selected.next() {
        let mut cluster = form_glyph_clusters(&glyphs[range], &chunk.text, span.font_size.get());
        if writing_mode == WritingMode::TopToBottom {
            apply_vertical_orientation(&mut cluster);
        }
        if has_letter_spacing && script_supports_letter_spacing(cluster.codepoint.script()) {
            apply_cluster_letter_spacing(&mut cluster, span, selected.peek().is_some());
        }
        if has_word_spacing && is_word_separator_characters(cluster.codepoint) {
            // 単語間隔は字間の負送り補正後に適用し、負の送り幅も許容する。
            cluster.advance += span.word_spacing;
        }
        clusters.push(cluster);
    }

    clusters
}

fn apply_length_adjust(chunk: &TextChunk, clusters: &mut [GlyphCluster]) {
    let is_horizontal = matches!(chunk.text_flow, TextFlow::Linear);

    for span in &chunk.spans {
        let target_width = match span.text_length {
            Some(v) => v,
            None => continue,
        };

        let (cluster_indexes, width) = span_cluster_width(span, clusters);

        if cluster_indexes.is_empty() {
            continue;
        }

        if span.length_adjust == LengthAdjust::Spacing {
            adjust_cluster_spacing(clusters, &cluster_indexes, target_width, width);
        } else {
            scale_span_clusters(
                clusters,
                &cluster_indexes,
                target_width / width,
                is_horizontal,
            );
        }
    }
}

fn adjust_cluster_spacing(
    clusters: &mut [GlyphCluster],
    indexes: &[usize],
    target_width: f32,
    width: f32,
) {
    let factor = if indexes.len() > 1 {
        (target_width - width) / (indexes.len() - 1) as f32
    } else {
        0.0
    };
    for &i in indexes {
        clusters[i].advance = clusters[i].width + factor;
        clusters[i].text_length_spacing = factor;
    }
}

fn span_cluster_width(span: &TextSpan, clusters: &[GlyphCluster]) -> (Vec<usize>, f32) {
    let mut cluster_indexes = Vec::new();
    for i in span.start..span.end {
        if let Some(index) = clusters.iter().position(|c| c.byte_idx.value() == i) {
            cluster_indexes.push(index);
        }
    }
    // 複数codepointのclusterを重複して数えない。
    cluster_indexes.sort();
    cluster_indexes.dedup();
    let mut width = 0.0;
    for i in &cluster_indexes {
        width += clusters[*i].width;
    }
    (cluster_indexes, width)
}

fn scale_span_clusters(
    clusters: &mut [GlyphCluster],
    indexes: &[usize],
    factor: f32,
    is_horizontal: bool,
) {
    if factor < 0.001 {
        return;
    }
    for i in indexes {
        clusters[*i].transform = clusters[*i].transform.pre_scale(factor, 1.0);
        if !is_horizontal {
            clusters[*i].advance *= factor;
            clusters[*i].width *= factor;
        }
    }
}

/// Rotates clusters according to
/// [Unicode Vertical_Orientation Property](https://www.unicode.org/reports/tr50/tr50-19.html).
fn apply_vertical_orientation(cluster: &mut GlyphCluster) {
    let orientation = unicode_vo::char_orientation(cluster.codepoint);
    if orientation == unicode_vo::Orientation::Upright {
        orient_upright_cluster(cluster);
    } else {
        // 回転する字形をベースライン間の中央へ移す。
        cluster.transform = cluster
            .transform
            .pre_translate(0.0, (cluster.ascent + cluster.descent) / 2.0);
    }
}

fn orient_upright_cluster(cluster: &mut GlyphCluster) {
    let mut ts = Transform::default();
    ts = ts.pre_translate(0.0, (cluster.ascent + cluster.descent) / 2.0);
    ts = ts.pre_rotate_at(
        -90.0,
        cluster.width / 2.0,
        -(cluster.ascent + cluster.descent) / 2.0,
    );
    cluster.path_transform = ts;
    cluster.ascent = cluster.width / 2.0;
    cluster.descent = -cluster.width / 2.0;
}

fn apply_cluster_letter_spacing(cluster: &mut GlyphCluster, span: &TextSpan, has_next: bool) {
    // 末尾の間隔は境界矩形と配置に影響させない。
    if has_next {
        cluster.advance += span.letter_spacing;
    }
    // 無効な負の送り幅はChromeと同じように消去する。
    if !cluster.advance.is_valid_length() {
        cluster.width = 0.0;
        cluster.advance = 0.0;
        cluster.glyphs = vec![];
    }
}

fn form_glyph_clusters(glyphs: &[Glyph], text: &str, font_size: f32) -> GlyphCluster {
    debug_assert!(!glyphs.is_empty());

    let mut positioned_glyphs = vec![];
    let (width, advance) = position_cluster_glyphs(glyphs, font_size, &mut positioned_glyphs);
    make_glyph_cluster(
        &glyphs[0],
        text,
        font_size,
        width,
        advance,
        positioned_glyphs,
    )
}

fn position_cluster_glyphs(
    glyphs: &[Glyph],
    font_size: f32,
    positioned_glyphs: &mut Vec<PositionedGlyph>,
) -> (f32, f32) {
    let mut x = 0.0;
    let mut width = 0.0;
    let mut advance = 0.0;
    for glyph in glyphs {
        let sx = glyph.font.scale(font_size);

        positioned_glyphs.push(position_glyph(glyph, x, font_size));

        x += glyph.width as f32;

        let glyph_width = glyph.width as f32 * sx;
        advance += glyph_width;
        if glyph_width > width {
            width = glyph_width;
        }
    }

    (width, advance)
}

fn make_glyph_cluster(
    glyph: &Glyph,
    text: &str,
    font_size: f32,
    width: f32,
    advance: f32,
    positioned_glyphs: Vec<PositionedGlyph>,
) -> GlyphCluster {
    let byte_idx = glyph.byte_idx;
    let font = glyph.font.clone();
    GlyphCluster {
        byte_idx,
        codepoint: byte_idx.char_from(text),
        width,
        advance,
        text_length_spacing: 0.0,
        ascent: font.ascent(font_size),
        descent: font.descent(font_size),
        has_relative_shift: false,
        transform: Transform::default(),
        path_transform: Transform::default(),
        glyphs: positioned_glyphs,
        visible: true,
    }
}

fn position_glyph(glyph: &Glyph, x: f32, font_size: f32) -> PositionedGlyph {
    // 後続glyphの位置はcluster内の現在位置からの相対値となる。
    let ts = Transform::from_translate(x + glyph.dx as f32, -glyph.dy as f32);
    PositionedGlyph {
        glyph_ts: ts,
        cluster_ts: Transform::default(),
        span_ts: Transform::default(),
        units_per_em: glyph.font.units_per_em.get(),
        font_size,
        font: glyph.font.id,
        text: glyph.text.clone(),
        id: glyph.id,
    }
}

pub(crate) trait DatabaseExt {
    fn load_font(
        &self,
        id: ID,
        variations: &[crate::svg_engine::usvg::FontVariation],
    ) -> Option<ResolvedFont>;
    fn has_char(&self, id: ID, c: char) -> bool;
}

// CSSのexは実glyphのx-heightを使い、行組み用の既存fallback metricは変更しない。
pub(crate) fn css_font_x_height(
    font: &Font,
    resolver: &FontResolver<'_>,
    fontdb: &mut Arc<Database>,
) -> Option<f32> {
    let id = (resolver.select_font)(font, fontdb)?;
    fontdb.with_face_data(id, |data, face_index| {
        let font_ref = skrifa::FontRef::from_index(data, face_index).ok()?;
        let location = font_ref.axes().location(
            font.variations
                .iter()
                .map(|variation| (Tag::from_be_bytes(variation.tag), variation.value)),
        );
        let metrics = font_ref.metrics(Size::unscaled(), &location);
        if !(16..=16384).contains(&metrics.units_per_em) {
            return None;
        }
        let x_height = metrics
            .x_height
            .filter(|height| height.is_finite() && *height > 0.0)
            .or_else(|| {
                let glyph = font_ref.charmap().map('x')?;
                let bounds = font_ref
                    .glyph_metrics(Size::unscaled(), &location)
                    .bounds(glyph)?;
                let height = bounds.y_max;
                (height.is_finite() && height > 0.0).then_some(height)
            })?;
        Some(x_height / metrics.units_per_em as f32)
    })?
}

impl DatabaseExt for Database {
    #[inline(never)]
    fn load_font(
        &self,
        id: ID,
        variations: &[crate::svg_engine::usvg::FontVariation],
    ) -> Option<ResolvedFont> {
        self.with_face_data(id, |data, face_index| -> Option<ResolvedFont> {
            let font = skrifa::FontRef::from_index(data, face_index).ok()?;

            // For variable fonts, metrics depend on the position in the design space,
            // so resolve the requested variations to a normalized location first.
            // Metrics are always in font units, so the size stays unscaled.
            let location = font.axes().location(
                variations
                    .iter()
                    .map(|v| (Tag::from_be_bytes(v.tag), v.value)),
            );
            let coords = location.coords();
            let metrics = font.metrics(Size::unscaled(), &location);

            resolved_font_from_metrics(&font, &metrics, coords, id)
        })?
    }

    #[inline(never)]
    fn has_char(&self, id: ID, c: char) -> bool {
        let res = self.with_face_data(id, |font_data, face_index| -> Option<bool> {
            let font = skrifa::FontRef::from_index(font_data, face_index).ok()?;
            let char_map = skrifa::charmap::Charmap::new(&font);
            char_map.map(c)?;
            Some(true)
        });

        res == Some(Some(true))
    }
}

fn resolved_font_from_metrics(
    font: &skrifa::FontRef<'_>,
    metrics: &skrifa::metrics::Metrics,
    coords: &[skrifa::instance::NormalizedCoord],
    id: ID,
) -> Option<ResolvedFont> {
    let (units_per_em, has_optical_sizing_axis) = resolved_font_identity(font, metrics)?;
    let ascent = metrics.ascent;
    let descent = metrics.descent;

    let x_height = resolved_x_height(metrics)?;

    let line_through = metrics.strikeout;
    let line_through_position = match line_through {
        Some(metrics) => metrics.offset.round() as i16,
        None => x_height.get() as i16 / 2,
    };

    let (underline_position, underline_thickness) = resolved_underline(metrics, units_per_em);
    let (subscript_offset, superscript_offset) =
        resolved_script_offsets(font, coords, units_per_em);

    Some(ResolvedFont {
        id,
        units_per_em,
        has_optical_sizing_axis,
        ascent: ascent.round() as i16,
        descent: descent.round() as i16,
        x_height,
        underline_position,
        underline_thickness,
        line_through_position,
        subscript_offset,
        superscript_offset,
    })
}

fn resolved_font_identity(
    font: &skrifa::FontRef<'_>,
    metrics: &skrifa::metrics::Metrics,
) -> Option<(NonZeroU16, bool)> {
    /* WHY: ttf-parserと同じunitsPerEm範囲を維持する。 */
    let units_per_em =
        NonZeroU16::new(metrics.units_per_em).filter(|value| (16..=16384).contains(&value.get()))?;
    /* WHY: opszの有無は同じfaceで既に取得したSkrifa metadataから保持する。 */
    let has_optical_sizing_axis = font
        .axes()
        .get_by_tag(crate::svg_engine::usvg::text::OPSZ)
        .is_some();

    Some((units_per_em, has_optical_sizing_axis))
}

fn resolved_x_height(metrics: &skrifa::metrics::Metrics) -> Option<NonZeroU16> {
    let x_height = metrics
        .x_height
        .filter(|x| *x > 0.0)
        .and_then(|x| u16::try_from(x.round() as i32).ok())
        .and_then(NonZeroU16::new);
    match x_height {
        Some(height) => Some(height),
        None => u16::try_from(((metrics.ascent - metrics.descent) * 0.45).round() as i32)
            .ok()
            .and_then(NonZeroU16::new),
    }
}

fn resolved_underline(
    metrics: &skrifa::metrics::Metrics,
    units_per_em: NonZeroU16,
) -> (i16, NonZeroU16) {
    match metrics.underline {
        Some(metrics) => {
            let thickness = u16::try_from(metrics.thickness.round() as i32)
                .ok()
                .and_then(NonZeroU16::new)
                .unwrap_or_else(|| NonZeroU16::new(units_per_em.get() / 12).unwrap());
            (metrics.offset.round() as i16, thickness)
        }
        None => (
            -(units_per_em.get() as i16) / 9,
            NonZeroU16::new(units_per_em.get() / 12).unwrap(),
        ),
    }
}

fn resolved_script_offsets(
    font: &skrifa::FontRef<'_>,
    coords: &[skrifa::instance::NormalizedCoord],
    units_per_em: NonZeroU16,
) -> (i16, i16) {
    let mut subscript_offset = (units_per_em.get() as f32 / 0.2).round() as i16;
    let mut superscript_offset = (units_per_em.get() as f32 / 0.4).round() as i16;
    if let Ok(os2) = font.os2() {
        subscript_offset = os2.y_subscript_y_offset();
        superscript_offset = os2.y_superscript_y_offset();
    }
    if let (Ok(mvar), true) = (font.mvar(), !coords.is_empty()) {
        use skrifa::raw::tables::mvar::tags::*;
        let metric_delta = |tag| mvar.metric_delta(tag, coords).unwrap_or_default().to_f32();
        subscript_offset += metric_delta(SBYO).round() as i16;
        superscript_offset += metric_delta(SPYO).round() as i16;
    }
    (subscript_offset, superscript_offset)
}

/// Text shaping with font fallback.
pub(crate) fn shape_text(
    text: &str,
    font: Arc<ResolvedFont>,
    small_caps: bool,
    apply_kerning: bool,
    requested_font: &crate::svg_engine::usvg::Font,
    font_size: f32,
    font_optical_sizing: crate::svg_engine::usvg::FontOpticalSizing,
    resolver: &FontResolver,
    fontdb: &mut Arc<fontdb::Database>,
) -> Vec<Glyph> {
    let context = FallbackContext {
        text,
        small_caps,
        apply_kerning,
        requested_font,
        font_size,
        font_optical_sizing,
        resolver,
    };
    let glyphs = shape_with_fallback(&context, font, fontdb);
    warn_missing_glyphs(text, &glyphs);
    glyphs
}

struct FallbackContext<'a> {
    text: &'a str,
    small_caps: bool,
    apply_kerning: bool,
    requested_font: &'a Font,
    font_size: f32,
    font_optical_sizing: FontOpticalSizing,
    resolver: &'a FontResolver<'a>,
}

fn shape_with_fallback(
    context: &FallbackContext<'_>,
    font: Arc<ResolvedFont>,
    fontdb: &mut Arc<fontdb::Database>,
) -> Vec<Glyph> {
    let mut glyphs = shape_with_selected_font(context, font.clone(), fontdb);
    let mut used_fonts = vec![font.id];
    loop {
        let missing = glyphs
            .iter()
            .find(|glyph| glyph.is_missing())
            .map(|glyph| glyph.byte_idx.char_from(context.text));
        let Some(c) = missing else { break };
        let Some(fallback_font) = select_fallback_font(context, c, &used_fonts, fontdb) else {
            break;
        };
        let fallback_glyphs = shape_with_selected_font(context, fallback_font.clone(), fontdb);
        if !merge_fallback_glyphs(&mut glyphs, fallback_glyphs) {
            break;
        }
        used_fonts.push(fallback_font.id);
    }
    glyphs
}

fn shape_with_selected_font(
    context: &FallbackContext<'_>,
    font: Arc<ResolvedFont>,
    fontdb: &fontdb::Database,
) -> Vec<Glyph> {
    shape_text_with_font(
        context.text,
        font,
        context.small_caps,
        context.apply_kerning,
        context.requested_font.variations(),
        context.font_size,
        context.font_optical_sizing,
        fontdb,
    )
    .unwrap_or_default()
}

fn select_fallback_font(
    context: &FallbackContext<'_>,
    c: char,
    used_fonts: &[ID],
    fontdb: &mut Arc<fontdb::Database>,
) -> Option<Arc<ResolvedFont>> {
    let selected = match &context.resolver.select_styled_fallback {
        Some(selector) => selector(context.requested_font, c, used_fonts, fontdb),
        None => (context.resolver.select_fallback)(c, used_fonts, fontdb),
    };
    selected
        .and_then(|id| fontdb.load_font(id, context.requested_font.variations()))
        .map(Arc::new)
}

fn merge_fallback_glyphs(glyphs: &mut Vec<Glyph>, fallback: Vec<Glyph>) -> bool {
    let all_matched = fallback.iter().all(|g| !g.is_missing());
    let same_boundaries = glyphs.len() == fallback.len()
        && glyphs
            .iter()
            .zip(&fallback)
            .all(|(primary, other)| primary.byte_idx == other.byte_idx);
    if all_matched && !same_boundaries {
        *glyphs = fallback;
        return false;
    }
    if glyphs.len() != fallback.len() {
        return false;
    }
    if all_matched && same_boundaries {
        replace_missing_clusters(glyphs, &fallback);
    } else {
        for i in 0..glyphs.len() {
            if glyphs[i].is_missing() && !fallback[i].is_missing() {
                glyphs[i] = fallback[i].clone();
            }
        }
    }
    true
}

fn warn_missing_glyphs(text: &str, glyphs: &[Glyph]) {
    for glyph in glyphs {
        if glyph.is_missing() {
            let c = glyph.byte_idx.char_from(text);
            log::warn!(
                "No fonts with a {}/U+{:X} character were found.",
                c,
                c as u32
            );
        }
    }
}

fn replace_missing_clusters(glyphs: &mut [Glyph], fallback_glyphs: &[Glyph]) {
    let mut start = 0;
    while start < glyphs.len() {
        let cluster = glyphs[start].byte_idx;
        let mut end = start + 1;
        while end < glyphs.len() && glyphs[end].byte_idx == cluster {
            end += 1;
        }
        if glyphs[start..end].iter().any(Glyph::is_missing) {
            glyphs[start..end].clone_from_slice(&fallback_glyphs[start..end]);
        }
        start = end;
    }
}

/// Converts a text into a list of glyph IDs.
///
/// This function will do the BIDI reordering and text shaping.
fn shape_text_with_font(
    text: &str,
    font: Arc<ResolvedFont>,
    small_caps: bool,
    apply_kerning: bool,
    variations: &[crate::svg_engine::usvg::FontVariation],
    font_size: f32,
    font_optical_sizing: crate::svg_engine::usvg::FontOpticalSizing,
    fontdb: &fontdb::Database,
) -> Option<Vec<Glyph>> {
    fontdb.with_face_data(font.id, |font_data, face_index| -> Option<Vec<Glyph>> {
        use svg_harfrust::{ShaperData, ShaperInstance};

        let hr_font = svg_harfrust::FontRef::from_index(font_data, face_index).ok()?;

        let variations = shaping_variations(variations, &font, font_size, font_optical_sizing);

        let bidi_info = unicode_bidi::BidiInfo::new(text, Some(unicode_bidi::Level::ltr()));
        let paragraph = &bidi_info.paragraphs[0];

        // A shaper instance is only needed to apply variations.
        let instance_data = (!variations.is_empty())
            .then(|| ShaperInstance::from_variations(&hr_font, &variations));
        let shaper_data = ShaperData::new(&hr_font);
        let shaper = shaper_data
            .shaper(&hr_font)
            .instance(instance_data.as_ref())
            .build();

        let features = shaping_features(small_caps, apply_kerning);

        Some(shape_visual_runs(
            text, &font, &shaper, &features, font_size, &bidi_info, paragraph,
        ))
    })?
}

fn shape_visual_runs(
    text: &str,
    font: &Arc<ResolvedFont>,
    shaper: &svg_harfrust::Shaper<'_>,
    features: &[svg_harfrust::Feature],
    font_size: f32,
    bidi_info: &unicode_bidi::BidiInfo<'_>,
    paragraph: &unicode_bidi::ParagraphInfo,
) -> Vec<Glyph> {
    let (levels, runs) = bidi_info.visual_runs(paragraph, paragraph.range.clone());
    let context = ShapingContext {
        text,
        font,
        shaper,
        features,
        font_size,
    };
    let mut glyphs = Vec::new();
    for run in &runs {
        append_shaped_run(&context, run, &levels, &mut glyphs);
    }
    glyphs
}

struct ShapingContext<'a> {
    text: &'a str,
    font: &'a Arc<ResolvedFont>,
    shaper: &'a svg_harfrust::Shaper<'a>,
    features: &'a [svg_harfrust::Feature],
    font_size: f32,
}

fn append_shaped_run(
    context: &ShapingContext<'_>,
    run: &std::ops::Range<usize>,
    levels: &[unicode_bidi::Level],
    glyphs: &mut Vec<Glyph>,
) {
    // visual_runs は非空の段落を非空の区間に分割する。
    let sub_text = &context.text[run.clone()];
    let ltr = levels[run.start].is_ltr();
    let direction = if ltr {
        svg_harfrust::Direction::LeftToRight
    } else {
        svg_harfrust::Direction::RightToLeft
    };
    let mut buffer = svg_harfrust::UnicodeBuffer::new();
    buffer.push_str(sub_text);
    buffer.set_direction(direction);
    // TODO: explicitly set language?
    buffer.guess_segment_properties();
    let output = context.shaper.shape(
        buffer,
        ShapeOptions::new()
            .point_size(Some(context.font_size))
            .features(context.features),
    );
    append_shaped_glyphs(&output, sub_text, run.start, ltr, context.font, glyphs);
}

fn append_shaped_glyphs(
    output: &svg_harfrust::GlyphBuffer,
    sub_text: &str,
    run_start: usize,
    ltr: bool,
    font: &Arc<ResolvedFont>,
    glyphs: &mut Vec<Glyph>,
) {
    let positions = output.glyph_positions();
    let infos = output.glyph_infos();
    for i in 0..output.len() {
        let pos = positions[i];
        let info = infos[i];
        let idx = run_start + info.cluster as usize;
        let start = info.cluster as usize;
        let end = if ltr {
            i.checked_add(1)
        } else {
            i.checked_sub(1)
        }
        .and_then(|last| infos.get(last))
        .map_or(sub_text.len(), |info| info.cluster as usize);
        glyphs.push(Glyph {
            byte_idx: ByteIndex::new(idx),
            cluster_len: end.checked_sub(start).unwrap_or(0),
            text: sub_text[start..end].to_string(),
            id: GlyphId(info.glyph_id),
            dx: pos.x_offset,
            dy: pos.y_offset,
            width: pos.x_advance,
            font: font.clone(),
        });
    }
}

fn shaping_variations(
    variations: &[crate::svg_engine::usvg::FontVariation],
    font: &ResolvedFont,
    font_size: f32,
    optical_sizing: FontOpticalSizing,
) -> Vec<svg_harfrust::Variation> {
    use svg_harfrust::{Tag, Variation};
    let mut variations: Vec<Variation> = variations
        .iter()
        .map(|v| Variation {
            tag: Tag::from_be_bytes(v.tag),
            value: v.value,
        })
        .collect();
    if optical_sizing == FontOpticalSizing::Auto {
        let has_explicit_opsz = variations.iter().any(|v| v.tag == *b"opsz");
        if !has_explicit_opsz && font.has_optical_sizing_axis {
            variations.push(Variation {
                tag: crate::svg_engine::usvg::text::OPSZ,
                value: font_size,
            });
        }
    }
    variations
}

fn shaping_features(small_caps: bool, apply_kerning: bool) -> Vec<svg_harfrust::Feature> {
    use svg_harfrust::{Feature, Tag};
    let mut features = Vec::new();
    if small_caps {
        features.push(Feature::new(Tag::new(b"smcp"), 1, ..));
    }
    if !apply_kerning {
        features.push(Feature::new(Tag::new(b"kern"), 0, ..));
    }
    features
}

/// An iterator over glyph clusters.
///
/// Input:  0 2 2 2 3 4 4 5 5
/// Result: 0 1     4 5   7
pub(crate) struct GlyphClusters<'a> {
    data: &'a [Glyph],
    idx: usize,
}

impl<'a> GlyphClusters<'a> {
    pub(crate) fn new(data: &'a [Glyph]) -> Self {
        GlyphClusters { data, idx: 0 }
    }
}

impl Iterator for GlyphClusters<'_> {
    type Item = (std::ops::Range<usize>, ByteIndex);

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx == self.data.len() {
            return None;
        }

        let start = self.idx;
        let cluster = self.data[self.idx].byte_idx;
        for g in &self.data[self.idx..] {
            if g.byte_idx != cluster {
                break;
            }

            self.idx += 1;
        }

        Some((start..self.idx, cluster))
    }
}

/// Checks that selected script supports letter spacing.
///
/// [In the CSS spec](https://www.w3.org/TR/css-text-3/#cursive-tracking).
///
/// The list itself is from: https://github.com/harfbuzz/harfbuzz/issues/64
pub(crate) fn script_supports_letter_spacing(script: unicode_script::Script) -> bool {
    use unicode_script::Script;

    !matches!(
        script,
        Script::Arabic
            | Script::Syriac
            | Script::Nko
            | Script::Manichaean
            | Script::Psalter_Pahlavi
            | Script::Mandaic
            | Script::Mongolian
            | Script::Phags_Pa
            | Script::Devanagari
            | Script::Bengali
            | Script::Gurmukhi
            | Script::Modi
            | Script::Sharada
            | Script::Syloti_Nagri
            | Script::Tirhuta
            | Script::Ogham
    )
}

/// A glyph.
///
/// Basically, a glyph ID and it's metrics.
#[derive(Clone)]
pub(crate) struct Glyph {
    /// The glyph ID in the font.
    pub(crate) id: GlyphId,

    /// Position in bytes in the original string.
    ///
    /// We use it to match a glyph with a character in the text chunk and therefore with the style.
    pub(crate) byte_idx: ByteIndex,

    // The length of the cluster in bytes.
    pub(crate) cluster_len: usize,

    /// The text from the original string that corresponds to that glyph.
    pub(crate) text: String,

    /// The glyph offset in font units.
    pub(crate) dx: i32,

    /// The glyph offset in font units.
    pub(crate) dy: i32,

    /// The glyph width / X-advance in font units.
    pub(crate) width: i32,

    /// Reference to the source font.
    ///
    /// Each glyph can have it's own source font.
    pub(crate) font: Arc<ResolvedFont>,
}

impl Glyph {
    fn is_missing(&self) -> bool {
        self.id.0 == 0
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ResolvedFont {
    pub(crate) id: ID,

    units_per_em: NonZeroU16,
    has_optical_sizing_axis: bool,

    // All values below are in font units.
    ascent: i16,
    descent: i16,
    x_height: NonZeroU16,

    underline_position: i16,
    underline_thickness: NonZeroU16,

    // line-through thickness should be the the same as underline thickness
    // according to the TrueType spec:
    // https://docs.microsoft.com/en-us/typography/opentype/spec/os2#ystrikeoutsize
    line_through_position: i16,

    subscript_offset: i16,
    superscript_offset: i16,
}

pub(crate) fn chunk_span_at(chunk: &TextChunk, byte_offset: ByteIndex) -> Option<&TextSpan> {
    chunk
        .spans
        .iter()
        .find(|&span| span_contains(span, byte_offset))
}

pub(crate) fn span_contains(span: &TextSpan, byte_offset: ByteIndex) -> bool {
    byte_offset.value() >= span.start && byte_offset.value() < span.end
}

/// Checks that the selected character is a word separator.
///
/// According to: https://www.w3.org/TR/css-text-3/#word-separator
pub(crate) fn is_word_separator_characters(c: char) -> bool {
    matches!(
        c as u32,
        0x0020 | 0x00A0 | 0x1361 | 0x010100 | 0x010101 | 0x01039F | 0x01091F
    )
}

impl ResolvedFont {
    #[inline]
    pub(crate) fn scale(&self, font_size: f32) -> f32 {
        font_size / self.units_per_em.get() as f32
    }

    #[inline]
    pub(crate) fn ascent(&self, font_size: f32) -> f32 {
        self.ascent as f32 * self.scale(font_size)
    }

    #[inline]
    pub(crate) fn descent(&self, font_size: f32) -> f32 {
        self.descent as f32 * self.scale(font_size)
    }

    #[inline]
    pub(crate) fn height(&self, font_size: f32) -> f32 {
        self.ascent(font_size) - self.descent(font_size)
    }

    #[inline]
    pub(crate) fn x_height(&self, font_size: f32) -> f32 {
        self.x_height.get() as f32 * self.scale(font_size)
    }

    #[inline]
    pub(crate) fn underline_position(&self, font_size: f32) -> f32 {
        self.underline_position as f32 * self.scale(font_size)
    }

    #[inline]
    fn underline_thickness(&self, font_size: f32) -> f32 {
        self.underline_thickness.get() as f32 * self.scale(font_size)
    }

    #[inline]
    pub(crate) fn line_through_position(&self, font_size: f32) -> f32 {
        self.line_through_position as f32 * self.scale(font_size)
    }

    #[inline]
    fn subscript_offset(&self, font_size: f32) -> f32 {
        self.subscript_offset as f32 * self.scale(font_size)
    }

    #[inline]
    fn superscript_offset(&self, font_size: f32) -> f32 {
        self.superscript_offset as f32 * self.scale(font_size)
    }

    fn dominant_baseline_shift(&self, baseline: DominantBaseline, font_size: f32) -> f32 {
        let alignment = match baseline {
            DominantBaseline::Auto | DominantBaseline::NoChange => AlignmentBaseline::Auto,
            DominantBaseline::UseScript => AlignmentBaseline::Auto, // unsupported
            DominantBaseline::ResetSize => AlignmentBaseline::Auto, // unsupported
            DominantBaseline::Ideographic => AlignmentBaseline::Ideographic,
            DominantBaseline::Alphabetic => AlignmentBaseline::Alphabetic,
            DominantBaseline::Hanging => AlignmentBaseline::Hanging,
            DominantBaseline::Mathematical => AlignmentBaseline::Mathematical,
            DominantBaseline::Central => AlignmentBaseline::Central,
            DominantBaseline::Middle => AlignmentBaseline::Middle,
            DominantBaseline::TextAfterEdge => AlignmentBaseline::TextAfterEdge,
            DominantBaseline::TextBeforeEdge => AlignmentBaseline::TextBeforeEdge,
        };

        self.alignment_baseline_shift(alignment, font_size)
    }

    // The `alignment-baseline` property is a mess.
    //
    // The SVG 1.1 spec (https://www.w3.org/TR/SVG11/text.html#BaselineAlignmentProperties)
    // goes on and on about what this property suppose to do, but doesn't actually explain
    // how it should be implemented. It's just a very verbose overview.
    //
    // As of Nov 2022, only Chrome and Safari support `alignment-baseline`. Firefox isn't.
    // Same goes for basically every SVG library in existence.
    // Meaning we have no idea how exactly it should be implemented.
    //
    // And even Chrome and Safari cannot agree on how to handle `baseline`, `after-edge`,
    // `text-after-edge` and `ideographic` variants. Producing vastly different output.
    //
    // As per spec, a proper implementation should get baseline values from the font itself,
    // using `BASE` and `bsln` TrueType tables. If those tables are not present,
    // we have to synthesize them (https://drafts.csswg.org/css-inline/#baseline-synthesis-fonts).
    // And in the worst case scenario simply fallback to hardcoded values.
    //
    // Also, most fonts do not provide `BASE` and `bsln` tables to begin with.
    //
    // Again, as of Nov 2022, Chrome does only the latter:
    // https://github.com/chromium/chromium/blob/main/third_party/blink/renderer/platform/fonts/font_metrics.cc#L153
    //
    // Since baseline TrueType tables parsing and baseline synthesis are pretty hard,
    // we do what Chrome does - use hardcoded values. And it seems like Safari does the same.
    //
    //
    // But that's not all! SVG 2 and CSS Inline Layout 3 did a baseline handling overhaul,
    // and it's far more complex now. Not sure if anyone actually supports it.
    fn alignment_baseline_shift(&self, alignment: AlignmentBaseline, font_size: f32) -> f32 {
        match alignment {
            AlignmentBaseline::Auto | AlignmentBaseline::Baseline => 0.0,
            AlignmentBaseline::BeforeEdge | AlignmentBaseline::TextBeforeEdge => {
                self.ascent(font_size)
            }
            AlignmentBaseline::Middle => self.x_height(font_size) * 0.5,
            AlignmentBaseline::Central => self.ascent(font_size) - self.height(font_size) * 0.5,
            AlignmentBaseline::AfterEdge | AlignmentBaseline::TextAfterEdge => {
                self.descent(font_size)
            }
            AlignmentBaseline::Ideographic => self.descent(font_size),
            AlignmentBaseline::Alphabetic => 0.0,
            AlignmentBaseline::Hanging => self.ascent(font_size) * 0.8,
            AlignmentBaseline::Mathematical => self.ascent(font_size) * 0.5,
        }
    }
}

pub(crate) type FontsCache = HashMap<Font, Arc<ResolvedFont>>;

/// A read-only text index in bytes.
///
/// Guarantee to be on a char boundary and in text bounds.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct ByteIndex(usize);

impl ByteIndex {
    fn new(i: usize) -> Self {
        ByteIndex(i)
    }

    pub(crate) fn value(&self) -> usize {
        self.0
    }

    /// Converts byte position into a code point position.
    pub(crate) fn code_point_at(&self, text: &str) -> usize {
        text.char_indices()
            .take_while(|(i, _)| *i != self.0)
            .count()
    }

    /// Converts byte position into a character.
    pub(crate) fn char_from(&self, text: &str) -> char {
        text[self.0..].chars().next().unwrap()
    }
}
