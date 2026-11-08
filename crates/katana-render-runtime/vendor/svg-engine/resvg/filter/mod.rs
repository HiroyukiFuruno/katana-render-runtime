// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::rc::Rc;

use rgb::{FromSlice, RGBA8};
use tiny_skia::IntRect;
use crate::svg_engine::usvg::{ApproxEqUlps, ApproxZeroUlps};

mod box_blur;
mod color_matrix;
mod component_transfer;
mod composite;
mod convolve_matrix;
mod displacement_map;
mod iir_blur;
mod lighting;
mod morphology;
mod turbulence;

// TODO: apply single primitive filters in-place

/// An image reference.
///
/// Image pixels should be stored in RGBA order.
///
/// Some filters will require premultiplied channels, some not.
/// See specific filter documentation for details.
#[derive(Clone, Copy)]
pub struct ImageRef<'a> {
    data: &'a [RGBA8],
    width: u32,
    height: u32,
}

impl<'a> ImageRef<'a> {
    /// Creates a new image reference.
    ///
    /// Doesn't clone the provided data.
    #[inline]
    pub fn new(width: u32, height: u32, data: &'a [RGBA8]) -> Self {
        ImageRef {
            data,
            width,
            height,
        }
    }

    #[inline]
    fn alpha_at(&self, x: u32, y: u32) -> i16 {
        self.data[(self.width * y + x) as usize].a as i16
    }
}

/// A mutable `ImageRef` variant.
pub struct ImageRefMut<'a> {
    data: &'a mut [RGBA8],
    width: u32,
    height: u32,
}

impl<'a> ImageRefMut<'a> {
    /// Creates a new mutable image reference.
    ///
    /// Doesn't clone the provided data.
    #[inline]
    pub fn new(width: u32, height: u32, data: &'a mut [RGBA8]) -> Self {
        ImageRefMut {
            data,
            width,
            height,
        }
    }

    #[inline]
    fn pixel_at(&self, x: u32, y: u32) -> RGBA8 {
        self.data[(self.width * y + x) as usize]
    }

    #[inline]
    fn pixel_at_mut(&mut self, x: u32, y: u32) -> &mut RGBA8 {
        &mut self.data[(self.width * y + x) as usize]
    }
}

#[derive(Debug)]
pub(crate) enum Error {
    InvalidRegion,
    NoResults,
}

trait PixmapExt: Sized {
    fn try_create(width: u32, height: u32) -> Result<tiny_skia::Pixmap, Error>;
    fn copy_region(&self, region: IntRect) -> Result<tiny_skia::Pixmap, Error>;
    fn clear(&mut self);
    fn into_srgb(&mut self);
    fn into_linear_rgb(&mut self);
}

impl PixmapExt for tiny_skia::Pixmap {
    fn try_create(width: u32, height: u32) -> Result<tiny_skia::Pixmap, Error> {
        tiny_skia::Pixmap::new(width, height).ok_or(Error::InvalidRegion)
    }

    fn copy_region(&self, region: IntRect) -> Result<tiny_skia::Pixmap, Error> {
        let rect = IntRect::from_xywh(region.x(), region.y(), region.width(), region.height())
            .ok_or(Error::InvalidRegion)?;
        self.clone_rect(rect).ok_or(Error::InvalidRegion)
    }

    fn clear(&mut self) {
        self.fill(tiny_skia::Color::TRANSPARENT);
    }

    fn into_srgb(&mut self) {
        demultiply_alpha(self.data_mut().as_rgba_mut());
        from_linear_rgb(self.data_mut().as_rgba_mut());
        multiply_alpha(self.data_mut().as_rgba_mut());
    }

    fn into_linear_rgb(&mut self) {
        demultiply_alpha(self.data_mut().as_rgba_mut());
        into_linear_rgb(self.data_mut().as_rgba_mut());
        multiply_alpha(self.data_mut().as_rgba_mut());
    }
}

/// Multiplies provided pixels alpha.
fn multiply_alpha(data: &mut [RGBA8]) {
    for p in data {
        let a = p.a as f32 / 255.0;
        p.b = (p.b as f32 * a + 0.5) as u8;
        p.g = (p.g as f32 * a + 0.5) as u8;
        p.r = (p.r as f32 * a + 0.5) as u8;
    }
}

/// Demultiplies provided pixels alpha.
fn demultiply_alpha(data: &mut [RGBA8]) {
    for p in data {
        let a = p.a as f32 / 255.0;
        p.b = (p.b as f32 / a + 0.5) as u8;
        p.g = (p.g as f32 / a + 0.5) as u8;
        p.r = (p.r as f32 / a + 0.5) as u8;
    }
}

/// Precomputed sRGB to LinearRGB table.
///
/// Since we are storing the result in `u8`, there is no need to compute those
/// values each time. Mainly because it's very expensive.
///
/// ```text
/// if (C_srgb <= 0.04045)
///     C_lin = C_srgb / 12.92;
///  else
///     C_lin = pow((C_srgb + 0.055) / 1.055, 2.4);
/// ```
///
/// Thanks to librsvg for the idea.
#[rustfmt::skip]
const SRGB_TO_LINEAR_RGB_TABLE: &[u8; 256] = &[
    0,   0,   0,   0,   0,   0,  0,    1,   1,   1,   1,   1,   1,   1,   1,   1,
    1,   1,   2,   2,   2,   2,  2,    2,   2,   2,   3,   3,   3,   3,   3,   3,
    4,   4,   4,   4,   4,   5,  5,    5,   5,   6,   6,   6,   6,   7,   7,   7,
    8,   8,   8,   8,   9,   9,  9,   10,  10,  10,  11,  11,  12,  12,  12,  13,
    13,  13,  14,  14,  15,  15,  16,  16,  17,  17,  17,  18,  18,  19,  19,  20,
    20,  21,  22,  22,  23,  23,  24,  24,  25,  25,  26,  27,  27,  28,  29,  29,
    30,  30,  31,  32,  32,  33,  34,  35,  35,  36,  37,  37,  38,  39,  40,  41,
    41,  42,  43,  44,  45,  45,  46,  47,  48,  49,  50,  51,  51,  52,  53,  54,
    55,  56,  57,  58,  59,  60,  61,  62,  63,  64,  65,  66,  67,  68,  69,  70,
    71,  72,  73,  74,  76,  77,  78,  79,  80,  81,  82,  84,  85,  86,  87,  88,
    90,  91,  92,  93,  95,  96,  97,  99, 100, 101, 103, 104, 105, 107, 108, 109,
    111, 112, 114, 115, 116, 118, 119, 121, 122, 124, 125, 127, 128, 130, 131, 133,
    134, 136, 138, 139, 141, 142, 144, 146, 147, 149, 151, 152, 154, 156, 157, 159,
    161, 163, 164, 166, 168, 170, 171, 173, 175, 177, 179, 181, 183, 184, 186, 188,
    190, 192, 194, 196, 198, 200, 202, 204, 206, 208, 210, 212, 214, 216, 218, 220,
    222, 224, 226, 229, 231, 233, 235, 237, 239, 242, 244, 246, 248, 250, 253, 255,
];

/// Precomputed LinearRGB to sRGB table.
///
/// Since we are storing the result in `u8`, there is no need to compute those
/// values each time. Mainly because it's very expensive.
///
/// ```text
/// if (C_lin <= 0.0031308)
///     C_srgb = C_lin * 12.92;
/// else
///     C_srgb = 1.055 * pow(C_lin, 1.0 / 2.4) - 0.055;
/// ```
///
/// Thanks to librsvg for the idea.
#[rustfmt::skip]
const LINEAR_RGB_TO_SRGB_TABLE: &[u8; 256] = &[
    0,  13,  22,  28,  34,  38,  42,  46,  50,  53,  56,  59,  61,  64,  66,  69,
    71,  73,  75,  77,  79,  81,  83,  85,  86,  88,  90,  92,  93,  95,  96,  98,
    99, 101, 102, 104, 105, 106, 108, 109, 110, 112, 113, 114, 115, 117, 118, 119,
    120, 121, 122, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136,
    137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 148, 149, 150, 151,
    152, 153, 154, 155, 155, 156, 157, 158, 159, 159, 160, 161, 162, 163, 163, 164,
    165, 166, 167, 167, 168, 169, 170, 170, 171, 172, 173, 173, 174, 175, 175, 176,
    177, 178, 178, 179, 180, 180, 181, 182, 182, 183, 184, 185, 185, 186, 187, 187,
    188, 189, 189, 190, 190, 191, 192, 192, 193, 194, 194, 195, 196, 196, 197, 197,
    198, 199, 199, 200, 200, 201, 202, 202, 203, 203, 204, 205, 205, 206, 206, 207,
    208, 208, 209, 209, 210, 210, 211, 212, 212, 213, 213, 214, 214, 215, 215, 216,
    216, 217, 218, 218, 219, 219, 220, 220, 221, 221, 222, 222, 223, 223, 224, 224,
    225, 226, 226, 227, 227, 228, 228, 229, 229, 230, 230, 231, 231, 232, 232, 233,
    233, 234, 234, 235, 235, 236, 236, 237, 237, 238, 238, 238, 239, 239, 240, 240,
    241, 241, 242, 242, 243, 243, 244, 244, 245, 245, 246, 246, 246, 247, 247, 248,
    248, 249, 249, 250, 250, 251, 251, 251, 252, 252, 253, 253, 254, 254, 255, 255,
];

/// Converts input pixel from sRGB into LinearRGB.
///
/// Provided pixels should have an **unpremultiplied alpha**.
///
/// RGB channels order of the input image doesn't matter, but alpha channel must be the last one.
fn into_linear_rgb(data: &mut [RGBA8]) {
    for p in data {
        p.r = SRGB_TO_LINEAR_RGB_TABLE[p.r as usize];
        p.g = SRGB_TO_LINEAR_RGB_TABLE[p.g as usize];
        p.b = SRGB_TO_LINEAR_RGB_TABLE[p.b as usize];
    }
}

/// Converts input pixel from LinearRGB into sRGB.
///
/// Provided pixels should have an **unpremultiplied alpha**.
///
/// RGB channels order of the input image doesn't matter, but alpha channel must be the last one.
fn from_linear_rgb(data: &mut [RGBA8]) {
    for p in data {
        p.r = LINEAR_RGB_TO_SRGB_TABLE[p.r as usize];
        p.g = LINEAR_RGB_TO_SRGB_TABLE[p.g as usize];
        p.b = LINEAR_RGB_TO_SRGB_TABLE[p.b as usize];
    }
}

// TODO: https://github.com/rust-lang/rust/issues/44095
#[inline]
fn f32_bound(min: f32, val: f32, max: f32) -> f32 {
    debug_assert!(min.is_finite());
    debug_assert!(max.is_finite());

    if val > max {
        max
    } else if val >= min {
        val
    } else {
        // Catches `val < min` as well as a NaN `val`.
        min
    }
}

#[derive(Clone)]
struct Image {
    /// Filter primitive result.
    ///
    /// All images have the same size which is equal to the current filter region.
    image: Rc<tiny_skia::Pixmap>,

    /// Image's region that has actual data.
    ///
    /// Region is in global coordinates and not in `image` one.
    ///
    /// Image's content outside this region will be transparent/cleared.
    ///
    /// Currently used only for `feTile`.
    region: IntRect,

    /// The current color space.
    color_space: crate::svg_engine::usvg::filter::ColorInterpolation,
}

impl Image {
    fn from_image(image: tiny_skia::Pixmap, color_space: crate::svg_engine::usvg::filter::ColorInterpolation) -> Self {
        let (w, h) = (image.width(), image.height());
        Image {
            image: Rc::new(image),
            region: IntRect::from_xywh(0, 0, w, h).unwrap(),
            color_space,
        }
    }

    fn into_color_space(
        self,
        color_space: crate::svg_engine::usvg::filter::ColorInterpolation,
    ) -> Result<Self, Error> {
        if color_space != self.color_space {
            let region = self.region;

            let mut image = self.take()?;

            match color_space {
                crate::svg_engine::usvg::filter::ColorInterpolation::SRGB => image.into_srgb(),
                crate::svg_engine::usvg::filter::ColorInterpolation::LinearRGB => image.into_linear_rgb(),
            }

            Ok(Image {
                image: Rc::new(image),
                region,
                color_space,
            })
        } else {
            Ok(self)
        }
    }

    fn take(self) -> Result<tiny_skia::Pixmap, Error> {
        match Rc::try_unwrap(self.image) {
            Ok(v) => Ok(v),
            Err(v) => Ok((*v).clone()),
        }
    }

    fn width(&self) -> u32 {
        self.image.width()
    }

    fn height(&self) -> u32 {
        self.image.height()
    }

    fn as_ref(&self) -> &tiny_skia::Pixmap {
        &self.image
    }
}

struct FilterResult {
    name: String,
    image: Image,
}

pub fn apply(
    filter: &crate::svg_engine::usvg::filter::Filter,
    ts: tiny_skia::Transform,
    source: &mut tiny_skia::Pixmap,
) {
    let result = apply_inner(filter, ts, source);
    let result = result.and_then(|(image, region)| apply_to_canvas(image, region, source));

    // Clear on error.
    if result.is_err() {
        source.fill(tiny_skia::Color::TRANSPARENT);
    }

    match result {
        Ok(_) => {}
        Err(Error::InvalidRegion) => {
            log::warn!("Filter has an invalid region.");
        }
        Err(Error::NoResults) => {}
    }
}

fn apply_inner(
    filter: &crate::svg_engine::usvg::filter::Filter,
    ts: crate::svg_engine::usvg::Transform,
    source: &tiny_skia::Pixmap,
) -> Result<(Image, IntRect), Error> {
    let (region, source_rect) = filter_region(filter, ts, source)?;
    let cropped_source = if region != source_rect {
        Some(source.copy_region(region)?)
    } else {
        None
    };
    let source = cropped_source.as_ref().unwrap_or(source);
    let output_region = region;
    let ts = ts.post_translate(-region.x() as f32, -region.y() as f32);
    let region = region.translate_to(0, 0).ok_or(Error::InvalidRegion)?;
    let image = apply_primitives(filter, ts, source, region)?;
    Ok((image, output_region))
}

fn filter_region(
    filter: &crate::svg_engine::usvg::filter::Filter,
    ts: crate::svg_engine::usvg::Transform,
    source: &tiny_skia::Pixmap,
) -> Result<(IntRect, IntRect), Error> {
    let region = filter.rect().transform(ts).map(|r| r.to_int_rect()).ok_or(Error::InvalidRegion)?;
    let source_rect =
        IntRect::from_xywh(0, 0, source.width(), source.height()).ok_or(Error::InvalidRegion)?;
    let region = crate::svg_engine::resvg::geom::fit_to_rect(region, source_rect).ok_or(Error::InvalidRegion)?;
    Ok((region, source_rect))
}

fn apply_primitives(
    filter: &crate::svg_engine::usvg::filter::Filter,
    ts: crate::svg_engine::usvg::Transform,
    source: &tiny_skia::Pixmap,
    region: IntRect,
) -> Result<Image, Error> {
    let mut results: Vec<FilterResult> = Vec::new();
    for primitive in filter.primitives() {
        apply_primitive(primitive, ts, source, region, &mut results)?;
    }
    results.pop().map(|result| result.image).ok_or(Error::NoResults)
}

fn apply_primitive(
    primitive: &crate::svg_engine::usvg::filter::Primitive,
    ts: crate::svg_engine::usvg::Transform,
    source: &tiny_skia::Pixmap,
    region: IntRect,
    results: &mut Vec<FilterResult>,
) -> Result<(), Error> {
    let subregion = primitive_subregion(primitive, ts, results)?;
    let context = FilterContext {
        region,
        subregion,
        ts,
        source,
        results,
        color_space: primitive.color_interpolation(),
    };
    let result = context.apply(primitive.kind())?;
    let result = if region != subregion {
        crop_primitive(result, primitive.kind(), region, subregion)?
    } else {
        result
    };
    results.push(FilterResult { name: primitive.result().to_string(), image: result });
    Ok(())
}

fn primitive_subregion(
    primitive: &crate::svg_engine::usvg::filter::Primitive,
    ts: crate::svg_engine::usvg::Transform,
    results: &[FilterResult],
) -> Result<IntRect, Error> {
    let mut subregion = primitive.rect().transform(ts).map(|r| r.to_int_rect()).ok_or(Error::InvalidRegion)?;
    if let crate::svg_engine::usvg::filter::Kind::Offset(fe) = primitive.kind() {
        if let crate::svg_engine::usvg::filter::Input::Reference(name) = fe.input() {
            if let Some(res) = results.iter().rev().find(|v| v.name == *name) {
                subregion = res.image.region;
            }
        }
    }
    Ok(subregion)
}

fn crop_primitive(
    result: Image,
    kind: &crate::svg_engine::usvg::filter::Kind,
    region: IntRect,
    subregion: IntRect,
) -> Result<Image, Error> {
    let subregion2 = if let crate::svg_engine::usvg::filter::Kind::Offset(..) = kind {
        region.translate_to(0, 0)
    } else {
        subregion.translate(-region.x(), -region.y())
    }
    .unwrap();
    let color_space = result.color_space;
    let pixmap = clear_outside_subregion(result, subregion2)?;
    Ok(Image { image: Rc::new(pixmap), region: subregion, color_space })
}

fn clear_outside_subregion(result: Image, subregion: IntRect) -> Result<tiny_skia::Pixmap, Error> {
    let mut paint = tiny_skia::Paint::default();
    paint.set_color(tiny_skia::Color::BLACK);
    paint.blend_mode = tiny_skia::BlendMode::Clear;
    let mut pixmap = result.take()?;
    let w = pixmap.width() as f32;
    let h = pixmap.height() as f32;
    if let Some(rect) = tiny_skia::Rect::from_xywh(0.0, 0.0, w, subregion.y() as f32) {
        pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
    }
    if let Some(rect) = tiny_skia::Rect::from_xywh(0.0, 0.0, subregion.x() as f32, h) {
        pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
    }
    if let Some(rect) = tiny_skia::Rect::from_xywh(subregion.right() as f32, 0.0, w, h) {
        pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
    }
    if let Some(rect) = tiny_skia::Rect::from_xywh(0.0, subregion.bottom() as f32, w, h) {
        pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
    }
    Ok(pixmap)
}

struct FilterContext<'a> {
    region: IntRect,
    subregion: IntRect,
    ts: crate::svg_engine::usvg::Transform,
    source: &'a tiny_skia::Pixmap,
    results: &'a [FilterResult],
    color_space: crate::svg_engine::usvg::filter::ColorInterpolation,
}

impl FilterContext<'_> {
    fn input(&self, input: &crate::svg_engine::usvg::filter::Input) -> Result<Image, Error> {
        get_input(input, self.region, self.source, self.results)
    }

    fn input_pair(
        &self,
        first: &crate::svg_engine::usvg::filter::Input,
        second: &crate::svg_engine::usvg::filter::Input,
    ) -> Result<(Image, Image), Error> {
        let input1 = self.input(first)?;
        let input2 = self.input(second)?;
        Ok((input1, input2))
    }

    fn apply(&self, kind: &crate::svg_engine::usvg::filter::Kind) -> Result<Image, Error> {
        use crate::svg_engine::usvg::filter::Kind;
        match kind {
            Kind::Blend(fe) => self.blend(fe),
            Kind::DropShadow(fe) => self.drop_shadow(fe),
            Kind::Flood(fe) => self.flood(fe),
            Kind::GaussianBlur(fe) => self.gaussian_blur(fe),
            Kind::Offset(fe) => self.offset(fe),
            Kind::Composite(fe) => self.composite(fe),
            Kind::Merge(fe) => self.merge(fe),
            Kind::Tile(fe) => self.tile(fe),
            Kind::Image(fe) => self.image(fe),
            Kind::ComponentTransfer(fe) => self.component_transfer(fe),
            Kind::ColorMatrix(fe) => self.color_matrix(fe),
            Kind::ConvolveMatrix(fe) => self.convolve_matrix(fe),
            Kind::Morphology(fe) => self.morphology(fe),
            Kind::DisplacementMap(fe) => self.displacement_map(fe),
            Kind::Turbulence(fe) => self.turbulence(fe),
            Kind::DiffuseLighting(fe) => self.diffuse_lighting(fe),
            Kind::SpecularLighting(fe) => self.specular_lighting(fe),
        }
    }

    fn drop_shadow(&self, fe: &crate::svg_engine::usvg::filter::DropShadow) -> Result<Image, Error> {
        apply_drop_shadow(fe, self.color_space, self.ts, self.input(fe.input())?)
    }

    fn flood(&self, fe: &crate::svg_engine::usvg::filter::Flood) -> Result<Image, Error> {
        apply_flood(fe, self.region)
    }

    fn gaussian_blur(&self, fe: &crate::svg_engine::usvg::filter::GaussianBlur) -> Result<Image, Error> {
        apply_blur(fe, self.color_space, self.ts, self.input(fe.input())?)
    }

    fn offset(&self, fe: &crate::svg_engine::usvg::filter::Offset) -> Result<Image, Error> {
        apply_offset(fe, self.ts, self.input(fe.input())?)
    }

    fn merge(&self, fe: &crate::svg_engine::usvg::filter::Merge) -> Result<Image, Error> {
        apply_merge(fe, self.color_space, self.region, self.source, self.results)
    }

    fn tile(&self, fe: &crate::svg_engine::usvg::filter::Tile) -> Result<Image, Error> {
        apply_tile(self.input(fe.input())?, self.region)
    }

    fn image(&self, fe: &crate::svg_engine::usvg::filter::Image) -> Result<Image, Error> {
        apply_image(fe, self.region, self.subregion, self.ts)
    }

    fn component_transfer(&self, fe: &crate::svg_engine::usvg::filter::ComponentTransfer) -> Result<Image, Error> {
        apply_component_transfer(fe, self.color_space, self.input(fe.input())?)
    }

    fn color_matrix(&self, fe: &crate::svg_engine::usvg::filter::ColorMatrix) -> Result<Image, Error> {
        apply_color_matrix(fe, self.color_space, self.input(fe.input())?)
    }

    fn convolve_matrix(&self, fe: &crate::svg_engine::usvg::filter::ConvolveMatrix) -> Result<Image, Error> {
        apply_convolve_matrix(fe, self.color_space, self.input(fe.input())?)
    }

    fn morphology(&self, fe: &crate::svg_engine::usvg::filter::Morphology) -> Result<Image, Error> {
        apply_morphology(fe, self.color_space, self.ts, self.input(fe.input())?)
    }

    fn turbulence(&self, fe: &crate::svg_engine::usvg::filter::Turbulence) -> Result<Image, Error> {
        apply_turbulence(fe, self.region, self.color_space, self.ts)
    }

    fn diffuse_lighting(&self, fe: &crate::svg_engine::usvg::filter::DiffuseLighting) -> Result<Image, Error> {
        apply_diffuse_lighting(fe, self.region, self.color_space, self.ts, self.input(fe.input())?)
    }

    fn specular_lighting(&self, fe: &crate::svg_engine::usvg::filter::SpecularLighting) -> Result<Image, Error> {
        apply_specular_lighting(fe, self.region, self.color_space, self.ts, self.input(fe.input())?)
    }

    fn blend(&self, fe: &crate::svg_engine::usvg::filter::Blend) -> Result<Image, Error> {
        let (input1, input2) = self.input_pair(fe.input1(), fe.input2())?;
        apply_blend(fe, self.color_space, self.region, input1, input2)
    }

    fn composite(&self, fe: &crate::svg_engine::usvg::filter::Composite) -> Result<Image, Error> {
        let (input1, input2) = self.input_pair(fe.input1(), fe.input2())?;
        apply_composite(fe, self.color_space, self.region, input1, input2)
    }

    fn displacement_map(&self, fe: &crate::svg_engine::usvg::filter::DisplacementMap) -> Result<Image, Error> {
        let (input1, input2) = self.input_pair(fe.input1(), fe.input2())?;
        apply_displacement_map(fe, self.region, self.color_space, self.ts, input1, input2)
    }
}

fn get_input(
    input: &crate::svg_engine::usvg::filter::Input,
    region: IntRect,
    source: &tiny_skia::Pixmap,
    results: &[FilterResult],
) -> Result<Image, Error> {
    match input {
        crate::svg_engine::usvg::filter::Input::SourceGraphic => {
            let image = source.clone();

            Ok(Image {
                image: Rc::new(image),
                region,
                color_space: crate::svg_engine::usvg::filter::ColorInterpolation::SRGB,
            })
        }
        crate::svg_engine::usvg::filter::Input::SourceAlpha => Ok(source_alpha(source, region)),
        crate::svg_engine::usvg::filter::Input::Reference(name) => {
            if let Some(v) = results.iter().rev().find(|v| v.name == *name) {
                Ok(v.image.clone())
            } else {
                // Technically unreachable.
                log::warn!("Unknown filter primitive reference '{}'.", name);
                get_input(&crate::svg_engine::usvg::filter::Input::SourceGraphic, region, source, results)
            }
        }
    }
}

fn source_alpha(source: &tiny_skia::Pixmap, region: IntRect) -> Image {
    let mut image = source.clone();
    // Set RGB to black. Keep alpha as is.
    for p in image.data_mut().as_rgba_mut() {
        p.r = 0;
        p.g = 0;
        p.b = 0;
    }
    Image {
        image: Rc::new(image),
        region,
        color_space: crate::svg_engine::usvg::filter::ColorInterpolation::SRGB,
    }
}

trait PixmapToImageRef<'a> {
    fn as_image_ref(&'a self) -> ImageRef<'a>;
    fn as_image_ref_mut(&'a mut self) -> ImageRefMut<'a>;
}

impl<'a> PixmapToImageRef<'a> for tiny_skia::Pixmap {
    fn as_image_ref(&'a self) -> ImageRef<'a> {
        ImageRef::new(self.width(), self.height(), self.data().as_rgba())
    }

    fn as_image_ref_mut(&'a mut self) -> ImageRefMut<'a> {
        ImageRefMut::new(self.width(), self.height(), self.data_mut().as_rgba_mut())
    }
}

fn apply_drop_shadow(
    fe: &crate::svg_engine::usvg::filter::DropShadow,
    cs: crate::svg_engine::usvg::filter::ColorInterpolation,
    ts: crate::svg_engine::usvg::Transform,
    input: Image,
) -> Result<Image, Error> {
    let (dx, dy) = match scale_coordinates(fe.dx(), fe.dy(), ts) {
        Some(v) => v,
        None => return Ok(input),
    };

    let mut pixmap = tiny_skia::Pixmap::try_create(input.width(), input.height())?;
    let input_pixmap = input.into_color_space(cs)?.take()?;
    let mut shadow_pixmap = input_pixmap.clone();
    prepare_drop_shadow(fe, cs, ts, &mut shadow_pixmap);

    pixmap.draw_pixmap(
        dx as i32,
        dy as i32,
        shadow_pixmap.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        tiny_skia::Transform::identity(),
        None,
    );

    pixmap.draw_pixmap(
        0,
        0,
        input_pixmap.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        tiny_skia::Transform::identity(),
        None,
    );

    Ok(Image::from_image(pixmap, cs))
}

fn prepare_drop_shadow(
    fe: &crate::svg_engine::usvg::filter::DropShadow,
    cs: crate::svg_engine::usvg::filter::ColorInterpolation,
    ts: crate::svg_engine::usvg::Transform,
    shadow_pixmap: &mut tiny_skia::Pixmap,
) {
    if let Some((std_dx, std_dy, use_box_blur)) =
        resolve_std_dev(fe.std_dev_x().get(), fe.std_dev_y().get(), ts)
    {
        if use_box_blur {
            box_blur::apply(std_dx, std_dy, shadow_pixmap.as_image_ref_mut());
        } else {
            iir_blur::apply(std_dx, std_dy, shadow_pixmap.as_image_ref_mut());
        }
    }

    flood_drop_shadow(fe, shadow_pixmap);

    match cs {
        crate::svg_engine::usvg::filter::ColorInterpolation::SRGB => shadow_pixmap.into_srgb(),
        crate::svg_engine::usvg::filter::ColorInterpolation::LinearRGB => shadow_pixmap.into_linear_rgb(),
    }
}

fn flood_drop_shadow(
    fe: &crate::svg_engine::usvg::filter::DropShadow,
    shadow_pixmap: &mut tiny_skia::Pixmap,
) {
    // flood
    let color = tiny_skia::Color::from_rgba8(
        fe.color().red,
        fe.color().green,
        fe.color().blue,
        fe.opacity().to_u8(),
    );
    for p in shadow_pixmap.pixels_mut() {
        let mut color = color;
        color.apply_opacity(p.alpha() as f32 / 255.0);
        *p = color.premultiply().to_color_u8();
    }

}

fn apply_blur(
    fe: &crate::svg_engine::usvg::filter::GaussianBlur,
    cs: crate::svg_engine::usvg::filter::ColorInterpolation,
    ts: crate::svg_engine::usvg::Transform,
    input: Image,
) -> Result<Image, Error> {
    let (std_dx, std_dy, use_box_blur) =
        match resolve_std_dev(fe.std_dev_x().get(), fe.std_dev_y().get(), ts) {
            Some(v) => v,
            None => return Ok(input),
        };

    let mut pixmap = input.into_color_space(cs)?.take()?;

    if use_box_blur {
        box_blur::apply(std_dx, std_dy, pixmap.as_image_ref_mut());
    } else {
        iir_blur::apply(std_dx, std_dy, pixmap.as_image_ref_mut());
    }

    Ok(Image::from_image(pixmap, cs))
}

fn apply_offset(
    fe: &crate::svg_engine::usvg::filter::Offset,
    ts: crate::svg_engine::usvg::Transform,
    input: Image,
) -> Result<Image, Error> {
    let (dx, dy) = match scale_coordinates(fe.dx(), fe.dy(), ts) {
        Some(v) => v,
        None => return Ok(input),
    };

    if dx.approx_zero_ulps(4) && dy.approx_zero_ulps(4) {
        return Ok(input);
    }

    let mut pixmap = tiny_skia::Pixmap::try_create(input.width(), input.height())?;
    pixmap.draw_pixmap(
        dx as i32,
        dy as i32,
        input.as_ref().as_ref(),
        &tiny_skia::PixmapPaint::default(),
        tiny_skia::Transform::identity(),
        None,
    );

    Ok(Image::from_image(pixmap, input.color_space))
}

fn apply_blend(
    fe: &crate::svg_engine::usvg::filter::Blend,
    cs: crate::svg_engine::usvg::filter::ColorInterpolation,
    region: IntRect,
    input1: Image,
    input2: Image,
) -> Result<Image, Error> {
    let input1 = input1.into_color_space(cs)?;
    let input2 = input2.into_color_space(cs)?;

    let mut pixmap = tiny_skia::Pixmap::try_create(region.width(), region.height())?;

    pixmap.draw_pixmap(
        0,
        0,
        input2.as_ref().as_ref(),
        &tiny_skia::PixmapPaint::default(),
        tiny_skia::Transform::identity(),
        None,
    );

    pixmap.draw_pixmap(
        0,
        0,
        input1.as_ref().as_ref(),
        &tiny_skia::PixmapPaint {
            blend_mode: crate::svg_engine::resvg::render::convert_blend_mode(fe.mode()),
            ..tiny_skia::PixmapPaint::default()
        },
        tiny_skia::Transform::identity(),
        None,
    );

    Ok(Image::from_image(pixmap, cs))
}

fn apply_composite(
    fe: &crate::svg_engine::usvg::filter::Composite,
    cs: crate::svg_engine::usvg::filter::ColorInterpolation,
    region: IntRect,
    input1: Image,
    input2: Image,
) -> Result<Image, Error> {
    use crate::svg_engine::usvg::filter::CompositeOperator as Operator;

    let input1 = input1.into_color_space(cs)?;
    let input2 = input2.into_color_space(cs)?;

    let mut pixmap = tiny_skia::Pixmap::try_create(region.width(), region.height())?;

    if let Operator::Arithmetic { k1, k2, k3, k4 } = fe.operator() {
        composite_arithmetic([k1, k2, k3, k4], input1, input2, &mut pixmap)?;
        return Ok(Image::from_image(pixmap, cs));
    }

    composite_blend(fe.operator(), input1, input2, &mut pixmap);
    Ok(Image::from_image(pixmap, cs))
}

fn composite_arithmetic(
    coefficients: [f32; 4],
    input1: Image,
    input2: Image,
    pixmap: &mut tiny_skia::Pixmap,
) -> Result<(), Error> {
    let [k1, k2, k3, k4] = coefficients;
    let pixmap1 = input1.take()?;
    let pixmap2 = input2.take()?;
    composite::arithmetic(
        k1,
        k2,
        k3,
        k4,
        pixmap1.as_image_ref(),
        pixmap2.as_image_ref(),
        pixmap.as_image_ref_mut(),
    );
    Ok(())
}

fn composite_blend(
    operator: crate::svg_engine::usvg::filter::CompositeOperator,
    input1: Image,
    input2: Image,
    pixmap: &mut tiny_skia::Pixmap,
) {
    pixmap.draw_pixmap(
        0,
        0,
        input2.as_ref().as_ref(),
        &tiny_skia::PixmapPaint::default(),
        tiny_skia::Transform::identity(),
        None,
    );

    let blend_mode = composite_blend_mode(operator);

    pixmap.draw_pixmap(
        0,
        0,
        input1.as_ref().as_ref(),
        &tiny_skia::PixmapPaint {
            blend_mode,
            ..tiny_skia::PixmapPaint::default()
        },
        tiny_skia::Transform::identity(),
        None,
    );

}

fn composite_blend_mode(operator: crate::svg_engine::usvg::filter::CompositeOperator) -> tiny_skia::BlendMode {
    use crate::svg_engine::usvg::filter::CompositeOperator as Operator;
    match operator {
        Operator::Over => tiny_skia::BlendMode::SourceOver,
        Operator::In => tiny_skia::BlendMode::SourceIn,
        Operator::Out => tiny_skia::BlendMode::SourceOut,
        Operator::Atop => tiny_skia::BlendMode::SourceAtop,
        Operator::Xor => tiny_skia::BlendMode::Xor,
        Operator::Lighter => tiny_skia::BlendMode::Plus,
        Operator::Arithmetic { .. } => tiny_skia::BlendMode::SourceOver,
    }
}

fn apply_merge(
    fe: &crate::svg_engine::usvg::filter::Merge,
    cs: crate::svg_engine::usvg::filter::ColorInterpolation,
    region: IntRect,
    source: &tiny_skia::Pixmap,
    results: &[FilterResult],
) -> Result<Image, Error> {
    let mut pixmap = tiny_skia::Pixmap::try_create(region.width(), region.height())?;

    for input in fe.inputs() {
        let input = get_input(input, region, source, results)?;
        let input = input.into_color_space(cs)?;
        pixmap.draw_pixmap(
            0,
            0,
            input.as_ref().as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::identity(),
            None,
        );
    }

    Ok(Image::from_image(pixmap, cs))
}

fn apply_flood(fe: &crate::svg_engine::usvg::filter::Flood, region: IntRect) -> Result<Image, Error> {
    let c = fe.color();

    let mut pixmap = tiny_skia::Pixmap::try_create(region.width(), region.height())?;
    pixmap.fill(tiny_skia::Color::from_rgba8(
        c.red,
        c.green,
        c.blue,
        fe.opacity().to_u8(),
    ));

    Ok(Image::from_image(
        pixmap,
        crate::svg_engine::usvg::filter::ColorInterpolation::SRGB,
    ))
}

fn apply_tile(input: Image, region: IntRect) -> Result<Image, Error> {
    let subregion = input.region.translate(-region.x(), -region.y()).unwrap();

    let tile_pixmap = input.image.copy_region(subregion)?;
    let mut paint = tiny_skia::Paint::default();
    paint.shader = tiny_skia::Pattern::new(
        tile_pixmap.as_ref(),
        tiny_skia::SpreadMode::Repeat,
        tiny_skia::FilterQuality::Bicubic,
        1.0,
        tiny_skia::Transform::from_translate(subregion.x() as f32, subregion.y() as f32),
    );

    let mut pixmap = tiny_skia::Pixmap::try_create(region.width(), region.height())?;
    let rect = tiny_skia::Rect::from_xywh(0.0, 0.0, region.width() as f32, region.height() as f32)
        .unwrap();
    pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);

    Ok(Image::from_image(
        pixmap,
        crate::svg_engine::usvg::filter::ColorInterpolation::SRGB,
    ))
}

fn apply_image(
    fe: &crate::svg_engine::usvg::filter::Image,
    region: IntRect,
    subregion: IntRect,
    ts: crate::svg_engine::usvg::Transform,
) -> Result<Image, Error> {
    let mut pixmap = tiny_skia::Pixmap::try_create(region.width(), region.height())?;

    let (sx, sy) = ts.get_scale();
    let transform = tiny_skia::Transform::from_row(
        sx,
        0.0,
        0.0,
        sy,
        subregion.x() as f32,
        subregion.y() as f32,
    );

    let ctx = crate::svg_engine::resvg::render::Context {
        max_bbox: tiny_skia::IntRect::from_xywh(0, 0, region.width(), region.height()).unwrap(),
    };

    crate::svg_engine::resvg::render::render_nodes(fe.root(), &ctx, transform, &mut pixmap.as_mut());

    Ok(Image::from_image(
        pixmap,
        crate::svg_engine::usvg::filter::ColorInterpolation::SRGB,
    ))
}

fn apply_component_transfer(
    fe: &crate::svg_engine::usvg::filter::ComponentTransfer,
    cs: crate::svg_engine::usvg::filter::ColorInterpolation,
    input: Image,
) -> Result<Image, Error> {
    let mut pixmap = input.into_color_space(cs)?.take()?;

    demultiply_alpha(pixmap.data_mut().as_rgba_mut());
    component_transfer::apply(fe, pixmap.as_image_ref_mut());
    multiply_alpha(pixmap.data_mut().as_rgba_mut());

    Ok(Image::from_image(pixmap, cs))
}

fn apply_color_matrix(
    fe: &crate::svg_engine::usvg::filter::ColorMatrix,
    cs: crate::svg_engine::usvg::filter::ColorInterpolation,
    input: Image,
) -> Result<Image, Error> {
    let mut pixmap = input.into_color_space(cs)?.take()?;

    demultiply_alpha(pixmap.data_mut().as_rgba_mut());
    color_matrix::apply(fe.kind(), pixmap.as_image_ref_mut());
    multiply_alpha(pixmap.data_mut().as_rgba_mut());

    Ok(Image::from_image(pixmap, cs))
}

fn apply_convolve_matrix(
    fe: &crate::svg_engine::usvg::filter::ConvolveMatrix,
    cs: crate::svg_engine::usvg::filter::ColorInterpolation,
    input: Image,
) -> Result<Image, Error> {
    let mut pixmap = input.into_color_space(cs)?.take()?;

    if fe.preserve_alpha() {
        demultiply_alpha(pixmap.data_mut().as_rgba_mut());
    }

    convolve_matrix::apply(fe, pixmap.as_image_ref_mut());

    Ok(Image::from_image(pixmap, cs))
}

fn apply_morphology(
    fe: &crate::svg_engine::usvg::filter::Morphology,
    cs: crate::svg_engine::usvg::filter::ColorInterpolation,
    ts: crate::svg_engine::usvg::Transform,
    input: Image,
) -> Result<Image, Error> {
    let (rx, ry) = match scale_coordinates(fe.radius_x().get(), fe.radius_y().get(), ts) {
        Some(v) => v,
        None => return Ok(input),
    };
    if !rx.is_finite() || !ry.is_finite() {
        return Err(Error::InvalidRegion);
    }
    if rx == 0.0 && ry == 0.0 {
        return Ok(input);
    }
    let mut pixmap = input.into_color_space(cs)?.take()?;
    morphology::apply(fe.operator(), rx, ry, pixmap.as_image_ref_mut());

    Ok(Image::from_image(pixmap, cs))
}

fn apply_displacement_map(
    fe: &crate::svg_engine::usvg::filter::DisplacementMap,
    region: IntRect,
    cs: crate::svg_engine::usvg::filter::ColorInterpolation,
    ts: crate::svg_engine::usvg::Transform,
    input1: Image,
    input2: Image,
) -> Result<Image, Error> {
    let pixmap1 = input1.into_color_space(cs)?.take()?;
    let mut pixmap2 = input2.into_color_space(cs)?.take()?;
    /* WHY: channel selectorはstraight RGBで評価し、alpha値はそのまま使う。 */
    demultiply_alpha(pixmap2.data_mut().as_rgba_mut());

    let mut pixmap = tiny_skia::Pixmap::try_create(region.width(), region.height())?;

    let (sx, sy) = match scale_coordinates(fe.scale(), fe.scale(), ts) {
        Some(v) => v,
        None => return Ok(Image::from_image(pixmap1, cs)),
    };

    displacement_map::apply(
        fe,
        sx,
        sy,
        pixmap1.as_image_ref(),
        pixmap2.as_image_ref(),
        pixmap.as_image_ref_mut(),
    );

    Ok(Image::from_image(pixmap, cs))
}

fn apply_turbulence(
    fe: &crate::svg_engine::usvg::filter::Turbulence,
    region: IntRect,
    cs: crate::svg_engine::usvg::filter::ColorInterpolation,
    ts: crate::svg_engine::usvg::Transform,
) -> Result<Image, Error> {
    let mut pixmap = tiny_skia::Pixmap::try_create(region.width(), region.height())?;

    let (sx, sy) = ts.get_scale();
    if sx.approx_zero_ulps(4) || sy.approx_zero_ulps(4) {
        return Ok(Image::from_image(pixmap, cs));
    }

    turbulence::apply(
        region.x() as f64 - ts.tx as f64,
        region.y() as f64 - ts.ty as f64,
        sx as f64,
        sy as f64,
        fe.base_frequency_x().get() as f64,
        fe.base_frequency_y().get() as f64,
        fe.num_octaves(),
        fe.seed(),
        fe.stitch_tiles(),
        fe.kind() == crate::svg_engine::usvg::filter::TurbulenceKind::FractalNoise,
        pixmap.as_image_ref_mut(),
    );

    multiply_alpha(pixmap.data_mut().as_rgba_mut());

    Ok(Image::from_image(pixmap, cs))
}

fn apply_diffuse_lighting(
    fe: &crate::svg_engine::usvg::filter::DiffuseLighting,
    region: IntRect,
    cs: crate::svg_engine::usvg::filter::ColorInterpolation,
    ts: crate::svg_engine::usvg::Transform,
    input: Image,
) -> Result<Image, Error> {
    let mut pixmap = tiny_skia::Pixmap::try_create(region.width(), region.height())?;

    let light_source = transform_light_source(fe.light_source(), region, ts);

    lighting::diffuse_lighting(
        fe,
        light_source,
        input.as_ref().as_image_ref(),
        pixmap.as_image_ref_mut(),
    );

    Ok(Image::from_image(pixmap, cs))
}

fn apply_specular_lighting(
    fe: &crate::svg_engine::usvg::filter::SpecularLighting,
    region: IntRect,
    cs: crate::svg_engine::usvg::filter::ColorInterpolation,
    ts: crate::svg_engine::usvg::Transform,
    input: Image,
) -> Result<Image, Error> {
    let mut pixmap = tiny_skia::Pixmap::try_create(region.width(), region.height())?;

    let light_source = transform_light_source(fe.light_source(), region, ts);

    lighting::specular_lighting(
        fe,
        light_source,
        input.as_ref().as_image_ref(),
        pixmap.as_image_ref_mut(),
    );

    Ok(Image::from_image(pixmap, cs))
}

// TODO: do not modify LightSource
fn transform_light_source(
    mut source: crate::svg_engine::usvg::filter::LightSource,
    region: IntRect,
    ts: crate::svg_engine::usvg::Transform,
) -> crate::svg_engine::usvg::filter::LightSource {
    use std::f32::consts::SQRT_2;
    use crate::svg_engine::usvg::filter::LightSource;

    match &mut source {
        LightSource::DistantLight(..) => {}
        LightSource::PointLight(light) => {
            let mut point = tiny_skia::Point::from_xy(light.x, light.y);
            ts.map_point(&mut point);
            light.x = point.x - region.x() as f32;
            light.y = point.y - region.y() as f32;
            light.z = light.z * (ts.sx * ts.sx + ts.sy * ts.sy).sqrt() / SQRT_2;
        }
        LightSource::SpotLight(light) => {
            transform_spot_light(light, region, ts);
        }
    }

    source
}

fn transform_spot_light(
    light: &mut crate::svg_engine::usvg::filter::SpotLight,
    region: IntRect,
    ts: crate::svg_engine::usvg::Transform,
) {
    let sz = (ts.sx * ts.sx + ts.sy * ts.sy).sqrt() / std::f32::consts::SQRT_2;
    let mut point = tiny_skia::Point::from_xy(light.x, light.y);
    ts.map_point(&mut point);
    light.x = point.x - region.x() as f32;
    light.y = point.y - region.y() as f32;
    light.z *= sz;
    let mut point = tiny_skia::Point::from_xy(light.points_at_x, light.points_at_y);
    ts.map_point(&mut point);
    light.points_at_x = point.x - region.x() as f32;
    light.points_at_y = point.y - region.y() as f32;
    light.points_at_z *= sz;
}

fn apply_to_canvas(
    input: Image,
    region: IntRect,
    pixmap: &mut tiny_skia::Pixmap,
) -> Result<(), Error> {
    let input = input.into_color_space(crate::svg_engine::usvg::filter::ColorInterpolation::SRGB)?;

    pixmap.fill(tiny_skia::Color::TRANSPARENT);
    pixmap.draw_pixmap(
        region.x(),
        region.y(),
        input.as_ref().as_ref(),
        &tiny_skia::PixmapPaint::default(),
        tiny_skia::Transform::identity(),
        None,
    );

    Ok(())
}

/// Calculates Gaussian blur sigmas for the current world transform.
///
/// If the last flag is set, then a box blur should be used. Or IIR otherwise.
fn resolve_std_dev(std_dx: f32, std_dy: f32, ts: crate::svg_engine::usvg::Transform) -> Option<(f64, f64, bool)> {
    let (mut std_dx, mut std_dy) = scale_coordinates(std_dx, std_dy, ts)?;

    // 'A negative value or a value of zero disables the effect of the given filter primitive
    // (i.e., the result is the filter input image).'
    if std_dx.approx_eq_ulps(&0.0, 4) && std_dy.approx_eq_ulps(&0.0, 4) {
        return None;
    }

    // Ignore tiny sigmas. In case of IIR blur it can lead to a transparent image.
    if std_dx < 0.05 {
        std_dx = 0.0;
    }

    if std_dy < 0.05 {
        std_dy = 0.0;
    }

    const BLUR_SIGMA_THRESHOLD: f32 = 2.0;
    // Check that the current feGaussianBlur filter can be applied using a box blur.
    let box_blur = std_dx >= BLUR_SIGMA_THRESHOLD || std_dy >= BLUR_SIGMA_THRESHOLD;

    Some((std_dx as f64, std_dy as f64, box_blur))
}

fn scale_coordinates(x: f32, y: f32, ts: crate::svg_engine::usvg::Transform) -> Option<(f32, f32)> {
    let (sx, sy) = ts.get_scale();
    Some((x * sx, y * sy))
}

#[cfg(test)]
crate::svg_engine::owned_svg_model_tests!("../../../../src/svg_engine_filter_internal_boundary_tests.rs");
