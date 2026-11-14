// Copyright 2022 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;
use std::{cmp::Ordering, mem};

use crate::svg_engine::usvg::GlyphId;
use fontdb::{Database, ID};
use skrifa::MetadataProvider;
use skrifa::Tag;
use skrifa::bitmap::{BitmapData, BitmapFormat, BitmapGlyph, BitmapStrikes};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::prelude::LocationRef;
use skrifa::raw::TableProvider as _;
use skrifa::raw::types::BoundingBox;
use svgtypes::Color;
use tiny_skia_path::{NonZeroRect, Size, Transform};
use xmlwriter::XmlWriter;

use crate::svg_engine::usvg::text::OPSZ;
use crate::svg_engine::usvg::text::colr::GlyphPainter;
use crate::svg_engine::usvg::*;

fn resolve_rendering_mode(text: &Text) -> ShapeRendering {
    match text.rendering_mode {
        TextRendering::OptimizeSpeed => ShapeRendering::CrispEdges,
        TextRendering::OptimizeLegibility => ShapeRendering::GeometricPrecision,
        TextRendering::GeometricPrecision => ShapeRendering::GeometricPrecision,
    }
}

/// Returns the effective variation settings for a glyph: the span's explicit
/// variations plus an automatically computed `opsz` value when
/// `font-optical-sizing: auto` is in effect and the font has an `opsz` axis
/// that wasn't set explicitly. This matches browser behavior
/// (CSS font-optical-sizing: auto).
fn effective_variations(
    cache: &mut Cache,
    span: &layout::Span,
    glyph: &layout::PositionedGlyph,
) -> Vec<FontVariation> {
    let mut variations = span.variations.clone();
    if span.font_optical_sizing == crate::svg_engine::usvg::FontOpticalSizing::Auto
        && !variations.iter().any(|v| &v.tag == b"opsz")
        && cache.has_opsz_axis(glyph.font)
    {
        variations.push(FontVariation::new(*b"opsz", glyph.font_size()));
    }
    variations
}

fn push_outline_paths(
    span: &layout::Span,
    builder: &mut tiny_skia_path::PathBuilder,
    new_children: &mut Vec<Node>,
    rendering_mode: ShapeRendering,
    abs_transform: Transform,
) {
    let builder = mem::replace(builder, tiny_skia_path::PathBuilder::new());

    if let Some(path) = builder.finish().and_then(|p| {
        Path::new(
            String::new(),
            span.visible,
            span.fill.clone(),
            span.stroke.clone(),
            span.paint_order,
            rendering_mode,
            Arc::new(p),
            abs_transform,
        )
    }) {
        new_children.push(Node::Path(Box::new(path)));
    }
}

pub(crate) fn flatten(text: &mut Text, cache: &mut Cache) -> Option<(Group, NonZeroRect)> {
    let mut new_children = vec![];
    let abs_transform = text.abs_transform;
    let rendering_mode = resolve_rendering_mode(text);
    for span in &text.layouted {
        flatten_span(
            span,
            cache,
            rendering_mode,
            abs_transform,
            &mut new_children,
        );
    }

    let mut group = Group {
        id: text.id.clone(),
        ..Group::empty()
    };

    for child in new_children {
        group.children.push(child);
    }

    group.calculate_bounding_boxes();
    let stroke_bbox = group.stroke_bounding_box().to_non_zero_rect()?;
    Some((group, stroke_bbox))
}

fn flatten_span(
    span: &layout::Span,
    cache: &mut Cache,
    rendering_mode: ShapeRendering,
    abs_transform: Transform,
    children: &mut Vec<Node>,
) {
    push_decoration(span.overline.as_ref(), rendering_mode, children);
    push_decoration(span.underline.as_ref(), rendering_mode, children);
    let mut flattener = SpanFlattener {
        span,
        builder: tiny_skia_path::PathBuilder::new(),
        children,
        rendering_mode,
        abs_transform,
    };
    for glyph in &span.positioned_glyphs {
        flattener.append_glyph(glyph, cache);
    }
    flattener.flush_outline();
    push_decoration(
        span.line_through.as_ref(),
        rendering_mode,
        flattener.children,
    );
}

fn push_decoration(path: Option<&Path>, rendering_mode: ShapeRendering, children: &mut Vec<Node>) {
    if let Some(path) = path {
        let mut path = path.clone();
        path.rendering_mode = rendering_mode;
        children.push(Node::Path(Box::new(path)));
    }
}

struct SpanFlattener<'a> {
    span: &'a layout::Span,
    builder: tiny_skia_path::PathBuilder,
    children: &'a mut Vec<Node>,
    rendering_mode: ShapeRendering,
    abs_transform: Transform,
}

impl SpanFlattener<'_> {
    fn append_glyph(&mut self, glyph: &layout::PositionedGlyph, cache: &mut Cache) {
        let variations = effective_variations(cache, self.span, glyph);
        if let Some(tree) = cache.fontdb_colr(glyph.font, glyph.id, &variations) {
            self.append_group(glyph.colr_transform(), Node::Group(Box::new(tree.root)));
        } else if let Some(node) = cache.fontdb_svg(glyph.font, glyph.id) {
            self.flush_outline();
            self.append_group(glyph.svg_transform(), node);
        } else if let Some(image) = cache.fontdb_raster(glyph.font, glyph.id) {
            self.flush_outline();
            let transform = bitmap_transform(&image, glyph);
            self.append_group(transform, Node::Image(Box::new(image.image)));
        } else {
            self.append_outline(glyph, &variations, cache);
        }
    }

    fn append_outline(
        &mut self,
        glyph: &layout::PositionedGlyph,
        variations: &[FontVariation],
        cache: &mut Cache,
    ) {
        let outline = cache.fontdb_outline(glyph.font, glyph.id, variations);
        if let Some(outline) = outline.and_then(|path| path.transform(glyph.outline_transform())) {
            self.builder.push_path(&outline);
        }
    }

    fn flush_outline(&mut self) {
        // 連続するoutlineをまとめ、glyphごとのPath生成を避ける。
        push_outline_paths(
            self.span,
            &mut self.builder,
            self.children,
            self.rendering_mode,
            self.abs_transform,
        );
    }

    fn append_group(&mut self, transform: Transform, child: Node) {
        let mut group = Group {
            transform,
            ..Group::empty()
        };
        group.children.push(child);
        group.calculate_bounding_boxes();
        self.children.push(Node::Group(Box::new(group)));
    }
}

fn bitmap_transform(image: &BitmapImage, glyph: &layout::PositionedGlyph) -> Transform {
    if image.is_sbix {
        glyph.sbix_transform(
            image.x as f32,
            image.y as f32,
            image.glyph_bbox.map(|bbox| bbox.x_min).unwrap_or(0) as f32,
            image.glyph_bbox.map(|bbox| bbox.y_min).unwrap_or(0) as f32,
            image.pixels_per_em as f32,
            image.image.size.height(),
        )
    } else {
        glyph.cbdt_transform(image.x as f32, image.y as f32, image.pixels_per_em as f32)
    }
}

#[derive(Default)]
struct PathBuilder {
    builder: tiny_skia_path::PathBuilder,
}

impl OutlinePen for PathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.builder.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(x, y);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.builder.quad_to(cx0, cy0, x, y);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.builder.cubic_to(cx0, cy0, cx1, cy1, x, y);
    }

    fn close(&mut self) {
        self.builder.close();
    }
}

pub(crate) trait DatabaseExt {
    fn outline(
        &self,
        id: ID,
        glyph_id: GlyphId,
        variations: &[crate::svg_engine::usvg::FontVariation],
    ) -> Option<tiny_skia_path::Path>;
    fn has_opsz_axis(&self, id: ID) -> bool;
    fn raster(&self, id: ID, glyph_id: GlyphId) -> Option<BitmapImage>;
    fn svg(&self, id: ID, glyph_id: GlyphId) -> Option<Node>;
    fn colr(
        &self,
        id: ID,
        glyph_id: GlyphId,
        variations: &[crate::svg_engine::usvg::FontVariation],
    ) -> Option<Tree>;
}

#[derive(Clone)]
pub(crate) struct BitmapImage {
    image: Image,
    x: i16,
    y: i16,
    pixels_per_em: u16,
    glyph_bbox: Option<BoundingBox<i16>>,
    is_sbix: bool,
}

impl DatabaseExt for Database {
    #[inline(never)]
    fn outline(
        &self,
        id: ID,
        glyph_id: GlyphId,
        variations: &[crate::svg_engine::usvg::FontVariation],
    ) -> Option<tiny_skia_path::Path> {
        self.with_face_data(id, |data, face_index| -> Option<tiny_skia_path::Path> {
            let font = skrifa::FontRef::from_index(data, face_index).ok()?;
            let outline = font.outline_glyphs().get(glyph_id.into())?;

            let mut builder = PathBuilder::default();

            let size = skrifa::prelude::Size::unscaled();
            // An empty variation list resolves to the default value of every
            // variation axis, which is what we want for non-variable fonts and
            // for variable fonts used without variations.
            let location = font.axes().location(
                variations
                    .iter()
                    .map(|v| (Tag::from_be_bytes(v.tag), v.value)),
            );
            outline
                .draw(DrawSettings::unhinted(size, &location), &mut builder)
                .ok()?;

            builder.builder.finish()
        })?
    }

    fn has_opsz_axis(&self, id: ID) -> bool {
        self.with_face_data(id, |data, face_index| -> Option<bool> {
            let font = skrifa::FontRef::from_index(data, face_index).ok()?;
            Some(font.axes().get_by_tag(OPSZ).is_some())
        })
        .flatten()
        .unwrap_or(false)
    }

    fn raster(&self, id: ID, glyph_id: GlyphId) -> Option<BitmapImage> {
        self.with_face_data(id, |data, face_index| -> Option<BitmapImage> {
            let font = skrifa::FontRef::from_index(data, face_index).ok()?;
            raster_glyph(&font, glyph_id)
        })?
    }

    fn svg(&self, id: ID, glyph_id: GlyphId) -> Option<Node> {
        // SEE: https://docs.rs/read-fonts/latest/read_fonts/tables/svg/type.Svg.html

        // TODO: Technically not 100% accurate because the SVG format in a OTF font
        // is actually a subset/superset of a normal SVG, but it seems to work fine
        // for Twitter Color Emoji, so might as well use what we already have.

        // TODO: Glyph records can contain the data for multiple glyphs. We should
        // add a cache so we don't need to reparse the data every time.
        self.with_face_data(id, |data, face_index| -> Option<Node> {
            let font = skrifa::FontRef::from_index(data, face_index).ok()?;
            svg_glyph(&font, glyph_id)
        })?
    }

    fn colr(
        &self,
        id: ID,
        glyph_id: GlyphId,
        variations: &[crate::svg_engine::usvg::FontVariation],
    ) -> Option<Tree> {
        self.with_face_data(id, |data, face_index| -> Option<Tree> {
            let font = skrifa::FontRef::from_index(data, face_index).ok()?;
            colr_glyph(&font, glyph_id, variations)
        })?
    }
}

fn raster_glyph(font: &skrifa::FontRef<'_>, glyph_id: GlyphId) -> Option<BitmapImage> {
    let bitmap_strikes = font.bitmap_strikes();
    // 最大の画像を選ぶため、スケールを指定しない。
    let size = skrifa::prelude::Size::unscaled();
    let location = LocationRef::default();
    let image = bitmap_strikes.glyph_for_size(size, glyph_id.into())?;
    match &image.data {
        BitmapData::Png(data) => png_bitmap_image(
            font,
            glyph_id,
            &image,
            data,
            &bitmap_strikes,
            size,
            location,
        ),
        BitmapData::Bgra(_) | BitmapData::Mask(_) => None,
    }
}

fn png_bitmap_image(
    font: &skrifa::FontRef<'_>,
    glyph_id: GlyphId,
    image: &BitmapGlyph<'_>,
    data: &[u8],
    strikes: &BitmapStrikes<'_>,
    size: skrifa::prelude::Size,
    location: LocationRef<'_>,
) -> Option<BitmapImage> {
    let metrics = font.glyph_metrics(size, location);
    let bounding_box = metrics.bounds(glyph_id.into()).map(|bbox| BoundingBox {
        x_min: bbox.x_min as i16,
        y_min: bbox.y_min as i16,
        x_max: bbox.x_max as i16,
        y_max: bbox.y_max as i16,
    });
    Some(BitmapImage {
        image: png_glyph_image(image, data)?,
        x: image.inner_bearing_x as i16,
        y: image.inner_bearing_y as i16,
        pixels_per_em: image.ppem_x as u16,
        glyph_bbox: bounding_box,
        is_sbix: strikes.format() == Some(BitmapFormat::Sbix),
    })
}

fn png_glyph_image(image: &BitmapGlyph<'_>, data: &[u8]) -> Option<Image> {
    let size = Size::from_wh(image.width as f32, image.height as f32)?;
    Some(Image {
        id: String::new(),
        visible: true,
        size,
        rendering_mode: ImageRendering::OptimizeQuality,
        kind: ImageKind::PNG(Arc::new(data.to_vec())),
        abs_transform: Transform::default(),
        abs_bounding_box: size.to_non_zero_rect(0.0, 0.0),
    })
}

fn svg_glyph(font: &skrifa::FontRef<'_>, glyph_id: GlyphId) -> Option<Node> {
    let svg_table = font.svg().ok()?;
    let document_list = svg_table.svg_document_list().ok()?;
    let records = document_list.document_records();
    /* WHY: glyph_dataと同じ探索で選んだレコードから、文書バイトとglyph範囲の両方を得る。 */
    let index = records
        .binary_search_by(|record| {
            if record.start_glyph_id.get().to_u32() > glyph_id.0 {
                Ordering::Greater
            } else if record.end_glyph_id.get().to_u32() < glyph_id.0 {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        })
        .ok()?;
    let doc_record = &records[index];
    let start = doc_record.svg_doc_offset();
    let end = start.checked_add(doc_record.svg_doc_length())?;
    let image_data = document_list
        .offset_data()
        .as_bytes()
        .get(start as usize..end as usize)?;
    let tree = Tree::from_data(image_data, &Options::default()).ok()?;
    svg_document_glyph(tree, glyph_id, doc_record.start_glyph_id == doc_record.end_glyph_id)
}

fn svg_document_glyph(tree: Tree, glyph_id: GlyphId, single_glyph: bool) -> Option<Node> {
    if single_glyph {
        Some(Node::Group(Box::new(tree.root)))
    } else {
        tree.node_by_id(&format!("glyph{}", glyph_id.0))
            .log_none(|| log::warn!("Failed to find SVG glyph node for glyph {}", glyph_id.0))
            .cloned()
    }
}

fn colr_glyph(
    font: &skrifa::FontRef<'_>,
    glyph_id: GlyphId,
    variations: &[FontVariation],
) -> Option<Tree> {
    let location = font.axes().location(
        variations
            .iter()
            .map(|variation| (Tag::from_be_bytes(variation.tag), variation.value)),
    );
    let mut svg = colr_svg_writer();
    let mut path_buf = String::with_capacity(256);
    svg.start_element("g");
    let mut painter =
        new_glyph_painter(font, LocationRef::from(&location), &mut svg, &mut path_buf);
    font.color_glyphs()
        .get(glyph_id.into())?
        .paint(&location, &mut painter)
        .ok()?;
    svg.end_element();
    Tree::from_data(svg.end_document().as_bytes(), &Options::default()).ok()
}

fn colr_svg_writer() -> XmlWriter {
    let mut svg = XmlWriter::new(xmlwriter::Options::default());
    svg.start_element("svg");
    svg.write_attribute("xmlns", "http://www.w3.org/2000/svg");
    svg.write_attribute("xmlns:xlink", "http://www.w3.org/1999/xlink");
    svg
}

fn new_glyph_painter<'a>(
    font: &'a skrifa::FontRef<'a>,
    location: LocationRef<'a>,
    svg: &'a mut XmlWriter,
    path_buf: &'a mut String,
) -> GlyphPainter<'a> {
    GlyphPainter {
        font,
        location,
        svg,
        path_buf,
        gradient_index: 1,
        clip_path_index: 1,
        foreground_color: Color::new_rgba(0, 0, 0, 255),
        transform: skrifa::color::Transform::default(),
        outline_transform: skrifa::color::Transform::default(),
        transforms_stack: vec![skrifa::color::Transform::default()],
        clip_stack: Vec::new(),
    }
}

#[cfg(test)]
crate::svg_engine::owned_svg_model_tests!("../../../../src/svg_engine_text_flatten_png_glyph_model_tests.rs");
