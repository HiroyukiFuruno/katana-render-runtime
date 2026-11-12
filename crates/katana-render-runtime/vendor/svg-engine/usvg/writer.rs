// Copyright 2023 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fmt::Display;
use std::io::Write;

use svgtypes::{FontFamily, parse_font_families};
use xmlwriter::XmlWriter;

use crate::svg_engine::usvg::parser::{AId, EId};
use crate::svg_engine::usvg::*;

impl Tree {
    /// Writes `usvg::Tree` back to SVG.
    pub fn to_string(&self, opt: &WriteOptions) -> String {
        convert(self, opt)
    }
}

/// Checks that type has a default value.
trait IsDefault: Default {
    /// Checks that type has a default value.
    fn is_default(&self) -> bool;
}

impl<T: Default + PartialEq + Copy> IsDefault for T {
    #[inline]
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// XML writing options.
#[derive(Clone, Debug)]
pub struct WriteOptions {
    /// Used to add a custom prefix to each element ID during writing.
    pub id_prefix: Option<String>,

    /// Do not convert text into paths.
    ///
    /// Default: false
    pub preserve_text: bool,

    /// Set the coordinates numeric precision.
    ///
    /// Smaller precision can lead to a malformed output in some cases.
    ///
    /// Default: 8
    pub coordinates_precision: u8,

    /// Set the transform values numeric precision.
    ///
    /// Smaller precision can lead to a malformed output in some cases.
    ///
    /// Default: 8
    pub transforms_precision: u8,

    /// Use single quote marks instead of double quote.
    ///
    /// # Examples
    ///
    /// Before:
    ///
    /// ```text
    /// <rect fill="red"/>
    /// ```
    ///
    /// After:
    ///
    /// ```text
    /// <rect fill='red'/>
    /// ```
    ///
    /// Default: disabled
    pub use_single_quote: bool,

    /// Set XML nodes indention.
    ///
    /// # Examples
    ///
    /// `Indent::None`
    /// Before:
    ///
    /// ```text
    /// <svg>
    ///     <rect fill="red"/>
    /// </svg>
    /// ```
    ///
    /// After:
    ///
    /// ```text
    /// <svg><rect fill="red"/></svg>
    /// ```
    ///
    /// Default: 4 spaces
    pub indent: Indent,

    /// Set XML attributes indention.
    ///
    /// # Examples
    ///
    /// `Indent::Spaces(2)`
    ///
    /// Before:
    ///
    /// ```text
    /// <svg>
    ///     <rect fill="red" stroke="black"/>
    /// </svg>
    /// ```
    ///
    /// After:
    ///
    /// ```text
    /// <svg>
    ///     <rect
    ///       fill="red"
    ///       stroke="black"/>
    /// </svg>
    /// ```
    ///
    /// Default: `None`
    pub attributes_indent: Indent,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            id_prefix: Default::default(),
            preserve_text: false,
            coordinates_precision: 8,
            transforms_precision: 8,
            use_single_quote: false,
            indent: Indent::Spaces(4),
            attributes_indent: Indent::None,
        }
    }
}

pub(crate) fn convert(tree: &Tree, opt: &WriteOptions) -> String {
    let mut xml = XmlWriter::new(xmlwriter::Options {
        use_single_quote: opt.use_single_quote,
        indent: opt.indent,
        attributes_indent: opt.attributes_indent,
    });

    xml.start_svg_element(EId::Svg);
    xml.write_svg_attribute(AId::Width, &tree.size.width());
    xml.write_svg_attribute(AId::Height, &tree.size.height());
    xml.write_attribute("xmlns", "http://www.w3.org/2000/svg");
    if has_xlink(&tree.root) {
        xml.write_attribute("xmlns:xlink", "http://www.w3.org/1999/xlink");
    }

    let has_text_paths = has_text_paths(&tree.root);
    if tree.has_defs_nodes() || has_text_paths {
        write_defs(tree, opt, &mut xml, has_text_paths);
    }

    write_elements(&tree.root, false, opt, &mut xml);

    xml.end_document()
}

fn write_filters(tree: &Tree, opt: &WriteOptions, xml: &mut XmlWriter) {
    let mut written_fe_image_nodes: Vec<String> = Vec::new();
    for filter in tree.filters() {
        write_filter_images(filter, &mut written_fe_image_nodes, opt, xml);

        xml.start_svg_element(EId::Filter);
        xml.write_id_attribute(filter.id(), opt);
        xml.write_rect_attrs(filter.rect);
        xml.write_units(
            AId::FilterUnits,
            Units::UserSpaceOnUse,
            Units::ObjectBoundingBox,
        );

        for fe in &filter.primitives {
            write_filter_primitive(fe, filter, opt, xml);
        }

        xml.end_element();
    }
}

fn write_filter_images(
    filter: &filter::Filter,
    written: &mut Vec<String>,
    opt: &WriteOptions,
    xml: &mut XmlWriter,
) {
    for fe in &filter.primitives {
        if let filter::Kind::Image(ref img) = fe.kind {
            let child = img.first_child();
            if !written.iter().any(|id| id == child.id()) {
                write_element(child, false, opt, xml);
                written.push(child.id().to_string());
            }
        }
    }
}

fn write_filter_primitive(
    fe: &filter::Primitive,
    filter: &filter::Filter,
    opt: &WriteOptions,
    xml: &mut XmlWriter,
) {
    match fe.kind {
        filter::Kind::DropShadow(_)
        | filter::Kind::GaussianBlur(_)
        | filter::Kind::Offset(_)
        | filter::Kind::Blend(_)
        | filter::Kind::Flood(_)
        | filter::Kind::Composite(_) => write_basic_filter_primitive(fe, filter, xml),
        filter::Kind::Merge(_)
        | filter::Kind::Tile(_)
        | filter::Kind::Image(_)
        | filter::Kind::ComponentTransfer(_)
        | filter::Kind::ColorMatrix(_)
        | filter::Kind::ConvolveMatrix(_) => write_geometry_filter_primitive(fe, filter, opt, xml),
        filter::Kind::Morphology(_)
        | filter::Kind::DisplacementMap(_)
        | filter::Kind::Turbulence(_)
        | filter::Kind::DiffuseLighting(_)
        | filter::Kind::SpecularLighting(_) => write_effect_filter_primitive(fe, filter, xml),
    }
}

fn write_basic_filter_primitive(
    fe: &filter::Primitive,
    filter: &filter::Filter,
    xml: &mut XmlWriter,
) {
    match fe.kind {
        filter::Kind::DropShadow(ref shadow) => write_filter_drop_shadow(shadow, filter, fe, xml),
        filter::Kind::GaussianBlur(ref blur) => write_filter_gaussian_blur(blur, filter, fe, xml),
        filter::Kind::Offset(ref offset) => write_filter_offset(offset, filter, fe, xml),
        filter::Kind::Blend(ref blend) => write_filter_blend(blend, filter, fe, xml),
        filter::Kind::Flood(ref flood) => write_filter_flood(flood, filter, fe, xml),
        filter::Kind::Composite(ref composite) => {
            write_filter_composite(composite, filter, fe, xml)
        }
        _ => unreachable!(),
    }
}

fn write_geometry_filter_primitive(
    fe: &filter::Primitive,
    filter: &filter::Filter,
    opt: &WriteOptions,
    xml: &mut XmlWriter,
) {
    match fe.kind {
        filter::Kind::Merge(ref merge) => write_filter_merge(merge, filter, fe, xml),
        filter::Kind::Tile(ref tile) => write_filter_tile(tile, filter, fe, xml),
        filter::Kind::Image(ref img) => write_filter_image(img, filter, fe, opt, xml),
        filter::Kind::ComponentTransfer(ref transfer) => {
            write_filter_component_transfer(transfer, filter, fe, xml)
        }
        filter::Kind::ColorMatrix(ref matrix) => write_filter_color_matrix(matrix, filter, fe, xml),
        filter::Kind::ConvolveMatrix(ref matrix) => {
            write_filter_convolve_matrix(matrix, filter, fe, xml)
        }
        _ => unreachable!(),
    }
}

fn write_effect_filter_primitive(
    fe: &filter::Primitive,
    filter: &filter::Filter,
    xml: &mut XmlWriter,
) {
    match fe.kind {
        filter::Kind::Morphology(ref morphology) => {
            write_filter_morphology(morphology, filter, fe, xml)
        }
        filter::Kind::DisplacementMap(ref map) => {
            write_filter_displacement_map(map, filter, fe, xml)
        }
        filter::Kind::Turbulence(ref turbulence) => {
            write_filter_turbulence(turbulence, filter, fe, xml)
        }
        filter::Kind::DiffuseLighting(ref light) => {
            write_filter_diffuse_lighting(light, filter, fe, xml)
        }
        filter::Kind::SpecularLighting(ref light) => {
            write_filter_specular_lighting(light, filter, fe, xml)
        }
        _ => unreachable!(),
    }
}

fn write_filter_drop_shadow(
    shadow: &filter::DropShadow,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeDropShadow);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_filter_input(AId::In, &shadow.input);
    xml.write_attribute_fmt(
        AId::StdDeviation.to_str(),
        format_args!("{} {}", shadow.std_dev_x.get(), shadow.std_dev_y.get()),
    );
    xml.write_svg_attribute(AId::Dx, &shadow.dx);
    xml.write_svg_attribute(AId::Dy, &shadow.dy);
    xml.write_color(AId::FloodColor, shadow.color);
    xml.write_svg_attribute(AId::FloodOpacity, &shadow.opacity.get());
    xml.write_svg_attribute(AId::Result, &fe.result);
    xml.end_element();
}

fn write_filter_gaussian_blur(
    blur: &filter::GaussianBlur,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeGaussianBlur);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_filter_input(AId::In, &blur.input);
    xml.write_attribute_fmt(
        AId::StdDeviation.to_str(),
        format_args!("{} {}", blur.std_dev_x.get(), blur.std_dev_y.get()),
    );
    xml.write_svg_attribute(AId::Result, &fe.result);
    xml.end_element();
}

fn write_filter_offset(
    offset: &filter::Offset,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeOffset);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_filter_input(AId::In, &offset.input);
    xml.write_svg_attribute(AId::Dx, &offset.dx);
    xml.write_svg_attribute(AId::Dy, &offset.dy);
    xml.write_svg_attribute(AId::Result, &fe.result);
    xml.end_element();
}

fn write_filter_blend(
    blend: &filter::Blend,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeBlend);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_filter_input(AId::In, &blend.input1);
    xml.write_filter_input(AId::In2, &blend.input2);
    xml.write_svg_attribute(AId::Mode, &blend.mode.to_string());
    xml.write_svg_attribute(AId::Result, &fe.result);
    xml.end_element();
}

fn write_filter_flood(
    flood: &filter::Flood,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeFlood);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_color(AId::FloodColor, flood.color);
    xml.write_svg_attribute(AId::FloodOpacity, &flood.opacity.get());
    xml.write_svg_attribute(AId::Result, &fe.result);
    xml.end_element();
}

fn write_filter_composite(
    composite: &filter::Composite,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeComposite);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_filter_input(AId::In, &composite.input1);
    xml.write_filter_input(AId::In2, &composite.input2);
    xml.write_svg_attribute(
        AId::Operator,
        match composite.operator {
            filter::CompositeOperator::Over => "over",
            filter::CompositeOperator::In => "in",
            filter::CompositeOperator::Out => "out",
            filter::CompositeOperator::Atop => "atop",
            filter::CompositeOperator::Xor => "xor",
            filter::CompositeOperator::Lighter => "lighter",
            filter::CompositeOperator::Arithmetic { .. } => "arithmetic",
        },
    );

    if let filter::CompositeOperator::Arithmetic { k1, k2, k3, k4 } = composite.operator {
        xml.write_svg_attribute(AId::K1, &k1);
        xml.write_svg_attribute(AId::K2, &k2);
        xml.write_svg_attribute(AId::K3, &k3);
        xml.write_svg_attribute(AId::K4, &k4);
    }

    xml.write_svg_attribute(AId::Result, &fe.result);
    xml.end_element();
}

fn write_filter_merge(
    merge: &filter::Merge,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeMerge);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_svg_attribute(AId::Result, &fe.result);
    for input in &merge.inputs {
        xml.start_svg_element(EId::FeMergeNode);
        xml.write_filter_input(AId::In, input);
        xml.end_element();
    }

    xml.end_element();
}

fn write_filter_tile(
    tile: &filter::Tile,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeTile);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_filter_input(AId::In, &tile.input);
    xml.write_svg_attribute(AId::Result, &fe.result);
    xml.end_element();
}

fn write_filter_image(
    img: &filter::Image,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    opt: &WriteOptions,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeImage);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    let child = img.first_child();
    let prefix = opt.id_prefix.as_deref().unwrap_or_default();
    xml.write_attribute_fmt("xlink:href", format_args!("#{}{}", prefix, child.id()));

    xml.write_svg_attribute(AId::Result, &fe.result);
    xml.end_element();
}

fn write_filter_component_transfer(
    transfer: &filter::ComponentTransfer,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeComponentTransfer);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_filter_input(AId::In, &transfer.input);
    xml.write_svg_attribute(AId::Result, &fe.result);

    xml.write_filter_transfer_function(EId::FeFuncR, &transfer.func_r);
    xml.write_filter_transfer_function(EId::FeFuncG, &transfer.func_g);
    xml.write_filter_transfer_function(EId::FeFuncB, &transfer.func_b);
    xml.write_filter_transfer_function(EId::FeFuncA, &transfer.func_a);

    xml.end_element();
}

fn write_filter_color_matrix(
    matrix: &filter::ColorMatrix,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeColorMatrix);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_filter_input(AId::In, &matrix.input);
    xml.write_svg_attribute(AId::Result, &fe.result);

    match matrix.kind {
        filter::ColorMatrixKind::Matrix(ref values) => {
            xml.write_svg_attribute(AId::Type, "matrix");
            xml.write_numbers(AId::Values, values);
        }
        filter::ColorMatrixKind::Saturate(value) => {
            xml.write_svg_attribute(AId::Type, "saturate");
            xml.write_svg_attribute(AId::Values, &value.get());
        }
        filter::ColorMatrixKind::HueRotate(angle) => {
            xml.write_svg_attribute(AId::Type, "hueRotate");
            xml.write_svg_attribute(AId::Values, &angle);
        }
        filter::ColorMatrixKind::LuminanceToAlpha => {
            xml.write_svg_attribute(AId::Type, "luminanceToAlpha");
        }
    }

    xml.end_element();
}

fn write_filter_convolve_matrix(
    matrix: &filter::ConvolveMatrix,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeConvolveMatrix);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_filter_input(AId::In, &matrix.input);
    xml.write_svg_attribute(AId::Result, &fe.result);

    xml.write_attribute_fmt(
        AId::Order.to_str(),
        format_args!("{} {}", matrix.matrix.columns, matrix.matrix.rows),
    );
    xml.write_numbers(AId::KernelMatrix, &matrix.matrix.data);
    xml.write_svg_attribute(AId::Divisor, &matrix.divisor.get());
    xml.write_svg_attribute(AId::Bias, &matrix.bias);
    xml.write_svg_attribute(AId::TargetX, &matrix.matrix.target_x);
    xml.write_svg_attribute(AId::TargetY, &matrix.matrix.target_y);
    write_convolve_edge_and_alpha(matrix, xml);

    xml.end_element();
}

fn write_convolve_edge_and_alpha(matrix: &filter::ConvolveMatrix, xml: &mut XmlWriter) {
    xml.write_svg_attribute(
        AId::EdgeMode,
        match matrix.edge_mode {
            filter::EdgeMode::None => "none",
            filter::EdgeMode::Duplicate => "duplicate",
            filter::EdgeMode::Wrap => "wrap",
        },
    );
    xml.write_svg_attribute(
        AId::PreserveAlpha,
        if matrix.preserve_alpha {
            "true"
        } else {
            "false"
        },
    );
}

fn write_filter_morphology(
    morphology: &filter::Morphology,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeMorphology);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_filter_input(AId::In, &morphology.input);
    xml.write_svg_attribute(AId::Result, &fe.result);

    xml.write_svg_attribute(
        AId::Operator,
        match morphology.operator {
            filter::MorphologyOperator::Erode => "erode",
            filter::MorphologyOperator::Dilate => "dilate",
        },
    );
    xml.write_attribute_fmt(
        AId::Radius.to_str(),
        format_args!(
            "{} {}",
            morphology.radius_x.get(),
            morphology.radius_y.get()
        ),
    );

    xml.end_element();
}

fn write_filter_displacement_map(
    map: &filter::DisplacementMap,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeDisplacementMap);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_filter_input(AId::In, &map.input1);
    xml.write_filter_input(AId::In2, &map.input2);
    xml.write_svg_attribute(AId::Result, &fe.result);

    xml.write_svg_attribute(AId::Scale, &map.scale);

    let mut write_channel = |c, aid| {
        xml.write_svg_attribute(
            aid,
            match c {
                filter::ColorChannel::R => "R",
                filter::ColorChannel::G => "G",
                filter::ColorChannel::B => "B",
                filter::ColorChannel::A => "A",
            },
        );
    };
    write_channel(map.x_channel_selector, AId::XChannelSelector);
    write_channel(map.y_channel_selector, AId::YChannelSelector);

    xml.end_element();
}

fn write_filter_turbulence(
    turbulence: &filter::Turbulence,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeTurbulence);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_svg_attribute(AId::Result, &fe.result);

    xml.write_attribute_fmt(
        AId::BaseFrequency.to_str(),
        format_args!(
            "{} {}",
            turbulence.base_frequency_x.get(),
            turbulence.base_frequency_y.get()
        ),
    );
    xml.write_svg_attribute(AId::NumOctaves, &turbulence.num_octaves);
    xml.write_svg_attribute(AId::Seed, &turbulence.seed);
    write_turbulence_options(turbulence, xml);

    xml.end_element();
}

fn write_turbulence_options(turbulence: &filter::Turbulence, xml: &mut XmlWriter) {
    xml.write_svg_attribute(
        AId::StitchTiles,
        match turbulence.stitch_tiles {
            true => "stitch",
            false => "noStitch",
        },
    );
    xml.write_svg_attribute(
        AId::Type,
        match turbulence.kind {
            filter::TurbulenceKind::FractalNoise => "fractalNoise",
            filter::TurbulenceKind::Turbulence => "turbulence",
        },
    );
}

fn write_filter_diffuse_lighting(
    light: &filter::DiffuseLighting,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeDiffuseLighting);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_filter_input(AId::In, &light.input);
    xml.write_svg_attribute(AId::Result, &fe.result);

    xml.write_svg_attribute(AId::SurfaceScale, &light.surface_scale);
    xml.write_svg_attribute(AId::DiffuseConstant, &light.diffuse_constant);
    xml.write_color(AId::LightingColor, light.lighting_color);
    write_light_source(&light.light_source, xml);

    xml.end_element();
}

fn write_filter_specular_lighting(
    light: &filter::SpecularLighting,
    filter: &filter::Filter,
    fe: &filter::Primitive,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::FeSpecularLighting);
    xml.write_filter_primitive_attrs(filter.rect(), fe);
    xml.write_filter_input(AId::In, &light.input);
    xml.write_svg_attribute(AId::Result, &fe.result);

    xml.write_svg_attribute(AId::SurfaceScale, &light.surface_scale);
    xml.write_svg_attribute(AId::SpecularConstant, &light.specular_constant);
    xml.write_svg_attribute(AId::SpecularExponent, &light.specular_exponent);
    xml.write_color(AId::LightingColor, light.lighting_color);
    write_light_source(&light.light_source, xml);

    xml.end_element();
}

fn write_defs(tree: &Tree, opt: &WriteOptions, xml: &mut XmlWriter, write_text_paths: bool) {
    xml.start_svg_element(EId::Defs);
    write_linear_gradients(tree, opt, xml);
    write_radial_gradients(tree, opt, xml);
    write_patterns(tree, opt, xml);
    if write_text_paths {
        write_text_path_paths(&tree.root, opt, xml);
    }
    write_filters(tree, opt, xml);
    write_clip_paths(tree, opt, xml);
    write_masks(tree, opt, xml);
    xml.end_element(); // end EId::Defs
}

fn write_linear_gradients(tree: &Tree, opt: &WriteOptions, xml: &mut XmlWriter) {
    for lg in tree.linear_gradients() {
        xml.start_svg_element(EId::LinearGradient);
        xml.write_id_attribute(lg.id(), opt);
        xml.write_svg_attribute(AId::X1, &lg.x1);
        xml.write_svg_attribute(AId::Y1, &lg.y1);
        xml.write_svg_attribute(AId::X2, &lg.x2);
        xml.write_svg_attribute(AId::Y2, &lg.y2);
        write_base_grad(&lg.base, opt, xml);
        xml.end_element();
    }
}

fn write_radial_gradients(tree: &Tree, opt: &WriteOptions, xml: &mut XmlWriter) {
    for rg in tree.radial_gradients() {
        xml.start_svg_element(EId::RadialGradient);
        xml.write_id_attribute(rg.id(), opt);
        xml.write_svg_attribute(AId::Cx, &rg.cx);
        xml.write_svg_attribute(AId::Cy, &rg.cy);
        xml.write_svg_attribute(AId::R, &rg.r.get());
        if rg.fr.get() != 0.0 {
            xml.write_svg_attribute(AId::Fr, &rg.fr.get());
        }
        xml.write_svg_attribute(AId::Fx, &rg.fx);
        xml.write_svg_attribute(AId::Fy, &rg.fy);
        write_base_grad(&rg.base, opt, xml);
        xml.end_element();
    }
}

fn write_patterns(tree: &Tree, opt: &WriteOptions, xml: &mut XmlWriter) {
    for pattern in tree.patterns() {
        xml.start_svg_element(EId::Pattern);
        xml.write_id_attribute(pattern.id(), opt);
        xml.write_rect_attrs(pattern.rect);
        xml.write_units(AId::PatternUnits, pattern.units, Units::ObjectBoundingBox);
        xml.write_units(
            AId::PatternContentUnits,
            pattern.content_units,
            Units::UserSpaceOnUse,
        );
        xml.write_transform(AId::PatternTransform, pattern.transform, opt);

        write_elements(&pattern.root, false, opt, xml);

        xml.end_element();
    }
}

fn write_clip_paths(tree: &Tree, opt: &WriteOptions, xml: &mut XmlWriter) {
    for clip in tree.clip_paths() {
        xml.start_svg_element(EId::ClipPath);
        xml.write_id_attribute(clip.id(), opt);
        xml.write_transform(AId::Transform, clip.transform, opt);

        if let Some(ref clip) = clip.clip_path {
            xml.write_func_iri(AId::ClipPath, clip.id(), opt);
        }

        write_elements(&clip.root, true, opt, xml);

        xml.end_element();
    }
}

fn write_masks(tree: &Tree, opt: &WriteOptions, xml: &mut XmlWriter) {
    for mask in tree.masks() {
        xml.start_svg_element(EId::Mask);
        xml.write_id_attribute(mask.id(), opt);
        if mask.kind == MaskType::Alpha {
            xml.write_svg_attribute(AId::MaskType, "alpha");
        }
        xml.write_units(
            AId::MaskUnits,
            Units::UserSpaceOnUse,
            Units::ObjectBoundingBox,
        );
        xml.write_rect_attrs(mask.rect);

        if let Some(ref mask) = mask.mask {
            xml.write_func_iri(AId::Mask, mask.id(), opt);
        }

        write_elements(&mask.root, false, opt, xml);

        xml.end_element();
    }
}

fn has_text_paths(parent: &Group) -> bool {
    for node in &parent.children {
        if let Node::Group(group) = node {
            if has_text_paths(group) {
                return true;
            }
        } else if let Node::Text(text) = node {
            for chunk in &text.chunks {
                if let TextFlow::Path(text_path) = &chunk.text_flow {
                    let path = text_path_element(text_path);
                    if path.is_some() {
                        return true;
                    }
                }
            }
        }
        let mut need_path = false;
        node.subroots(|subroot| {
            if !need_path && has_text_paths(subroot) {
                need_path = true;
            }
        });
        if need_path {
            return true;
        }
    }
    false
}

/// Write the `path` elements for text paths.
fn write_text_path_paths(parent: &Group, opt: &WriteOptions, xml: &mut XmlWriter) {
    for node in &parent.children {
        if let Node::Group(group) = node {
            write_text_path_paths(group, opt, xml);
        } else if let Node::Text(text) = node {
            for chunk in &text.chunks {
                if let TextFlow::Path(text_path) = &chunk.text_flow {
                    let path = text_path_element(text_path);
                    if let Some(path) = &path {
                        write_path(path, false, Transform::default(), None, opt, xml);
                    }
                }
            }
        }

        node.subroots(|subroot| write_text_path_paths(subroot, opt, xml));
    }
}

fn text_path_element(text_path: &TextPath) -> Option<Path> {
    Path::new(
        text_path.id().to_string(),
        true,
        None,
        None,
        PaintOrder::default(),
        ShapeRendering::default(),
        text_path.path.clone(),
        Transform::default(),
    )
}

fn write_elements(parent: &Group, is_clip_path: bool, opt: &WriteOptions, xml: &mut XmlWriter) {
    for n in &parent.children {
        write_element(n, is_clip_path, opt, xml);
    }
}

fn write_image_element(img: &Image, opt: &WriteOptions, xml: &mut XmlWriter) {
    xml.start_svg_element(EId::Image);
    if !img.id.is_empty() {
        xml.write_id_attribute(&img.id, opt);
    }
    xml.write_svg_attribute(AId::Width, &img.size().width());
    xml.write_svg_attribute(AId::Height, &img.size().height());
    xml.write_visibility(img.visible);
    write_image_rendering(img.rendering_mode, xml);
    xml.write_image_data(&img.kind);
    xml.end_element();
}

fn write_image_rendering(mode: ImageRendering, xml: &mut XmlWriter) {
    match mode {
        ImageRendering::OptimizeQuality => {}
        ImageRendering::OptimizeSpeed => {
            xml.write_svg_attribute(AId::ImageRendering, "optimizeSpeed");
        }
        ImageRendering::Smooth => {
            xml.write_attribute(AId::Style.to_str(), "image-rendering:smooth");
        }
        ImageRendering::HighQuality => {
            xml.write_attribute(AId::Style.to_str(), "image-rendering:high-quality");
        }
        ImageRendering::CrispEdges => {
            xml.write_attribute(AId::Style.to_str(), "image-rendering:crisp-edges");
        }
        ImageRendering::Pixelated => {
            xml.write_attribute(AId::Style.to_str(), "image-rendering:pixelated");
        }
    }
}

fn write_element(node: &Node, is_clip_path: bool, opt: &WriteOptions, xml: &mut XmlWriter) {
    match node {
        Node::Path(p) => {
            write_path(p, is_clip_path, Transform::default(), None, opt, xml);
        }
        Node::Image(img) => write_image_element(img, opt, xml),
        Node::Group(g) => {
            write_group_element(g, is_clip_path, opt, xml);
        }
        Node::Text(text) => write_text_element(text, is_clip_path, opt, xml),
    }
}

fn write_text_element(text: &Text, is_clip_path: bool, opt: &WriteOptions, xml: &mut XmlWriter) {
    if opt.preserve_text {
        write_preserved_text(text, is_clip_path, opt, xml);
    } else {
        write_group_element(text.flattened(), is_clip_path, opt, xml);
    }
}

fn write_preserved_text(text: &Text, is_clip_path: bool, opt: &WriteOptions, xml: &mut XmlWriter) {
    xml.start_svg_element(EId::Text);
    write_text_attributes(text, opt, xml);
    xml.set_preserve_whitespaces(true);
    for chunk in &text.chunks {
        write_text_chunk(chunk, is_clip_path, opt, xml);
    }
    xml.end_element();
    xml.set_preserve_whitespaces(false);
}

fn write_text_attributes(text: &Text, opt: &WriteOptions, xml: &mut XmlWriter) {
    if !text.id.is_empty() {
        xml.write_id_attribute(&text.id, opt);
    }

    xml.write_attribute("xml:space", "preserve");

    write_text_modes(text, xml);

    if text.rotate.iter().any(|r| *r != 0.0) {
        xml.write_numbers(AId::Rotate, &text.rotate);
    }

    if text.dx.iter().any(|dx| *dx != 0.0) {
        xml.write_numbers(AId::Dx, &text.dx);
    }

    if text.dy.iter().any(|dy| *dy != 0.0) {
        xml.write_numbers(AId::Dy, &text.dy);
    }
}

fn write_text_modes(text: &Text, xml: &mut XmlWriter) {
    match text.writing_mode {
        WritingMode::LeftToRight => {}
        WritingMode::TopToBottom => xml.write_svg_attribute(AId::WritingMode, "tb"),
    }

    match text.rendering_mode {
        TextRendering::OptimizeSpeed => {
            xml.write_svg_attribute(AId::TextRendering, "optimizeSpeed");
        }
        TextRendering::GeometricPrecision => {
            xml.write_svg_attribute(AId::TextRendering, "geometricPrecision");
        }
        TextRendering::OptimizeLegibility => {}
    }
}

fn write_text_chunk(
    chunk: &TextChunk,
    is_clip_path: bool,
    opt: &WriteOptions,
    xml: &mut XmlWriter,
) {
    start_text_path(chunk, opt, xml);

    xml.start_svg_element(EId::Tspan);

    if let Some(x) = chunk.x {
        xml.write_svg_attribute(AId::X, &x);
    }

    if let Some(y) = chunk.y {
        xml.write_svg_attribute(AId::Y, &y);
    }

    match chunk.anchor {
        TextAnchor::Start => {}
        TextAnchor::Middle => xml.write_svg_attribute(AId::TextAnchor, "middle"),
        TextAnchor::End => xml.write_svg_attribute(AId::TextAnchor, "end"),
    }
    for span in &chunk.spans {
        write_decorated_span(chunk, span, is_clip_path, opt, xml);
    }
    xml.end_element();

    // End textPath element
    if matches!(&chunk.text_flow, TextFlow::Path(_)) {
        xml.end_element();
    }
}

fn start_text_path(chunk: &TextChunk, opt: &WriteOptions, xml: &mut XmlWriter) {
    if let TextFlow::Path(text_path) = &chunk.text_flow {
        xml.start_svg_element(EId::TextPath);
        let prefix = opt.id_prefix.as_deref().unwrap_or_default();
        xml.write_attribute_fmt("xlink:href", format_args!("#{}{}", prefix, text_path.id()));
        if text_path.start_offset != 0.0 {
            xml.write_svg_attribute(AId::StartOffset, &text_path.start_offset);
        }
    }
}

fn write_decorated_span(
    chunk: &TextChunk,
    span: &TextSpan,
    is_clip_path: bool,
    opt: &WriteOptions,
    xml: &mut XmlWriter,
) {
    let decorations: Vec<_> = [
        ("underline", &span.decoration.underline),
        ("line-through", &span.decoration.line_through),
        ("overline", &span.decoration.overline),
    ]
    .iter()
    .filter_map(|&(key, option_value)| option_value.as_ref().map(|value| (key, value)))
    .collect();

    // Decorations need to be dumped BEFORE we write the actual span data
    // (so that for example stroke color of span doesn't affect the text
    // itself while baseline shifts need to be written after (since they are
    // affected by the font size)
    write_decoration_starts(&decorations, opt, xml);

    write_span(is_clip_path, opt, xml, chunk, span);

    // End for each tspan we needed to create for decorations
    for _ in &decorations {
        xml.end_element();
    }
}

fn write_decoration_starts(
    decorations: &[(&str, &TextDecorationStyle)],
    opt: &WriteOptions,
    xml: &mut XmlWriter,
) {
    for (deco_name, deco) in decorations {
        xml.start_svg_element(EId::Tspan);
        xml.write_svg_attribute(AId::TextDecoration, deco_name);
        write_fill(&deco.fill, false, opt, xml);
        write_stroke(&deco.stroke, opt, xml);
    }
}

fn write_group_element(g: &Group, is_clip_path: bool, opt: &WriteOptions, xml: &mut XmlWriter) {
    if is_clip_path {
        // SVGのclipPath内にはgroupを出力できないため、子要素を直接出力する。
        // usvgでは子のclip-pathがgroupに載るので、パス出力時に引き継ぐ。
        write_clipped_group_children(g, is_clip_path, opt, xml);
        return;
    }

    xml.start_svg_element(EId::G);
    write_group_attributes(g, opt, xml);
    write_elements(g, false, opt, xml);
    xml.end_element();
}

fn write_clipped_group_children(
    g: &Group,
    is_clip_path: bool,
    opt: &WriteOptions,
    xml: &mut XmlWriter,
) {
    for child in &g.children {
        match child {
            Node::Group(child_group) => write_group_element(child_group, is_clip_path, opt, xml),
            Node::Path(child_path) => {
                let clip_id = g.clip_path.as_ref().map(|cp| cp.id().to_string());
                write_path(
                    child_path,
                    is_clip_path,
                    g.transform,
                    clip_id.as_deref(),
                    opt,
                    xml,
                );
            }
            _ => {}
        }
    }
}

fn write_group_attributes(g: &Group, opt: &WriteOptions, xml: &mut XmlWriter) {
    if !g.id.is_empty() {
        xml.write_id_attribute(&g.id, opt);
    };

    if let Some(ref clip) = g.clip_path {
        xml.write_func_iri(AId::ClipPath, clip.id(), opt);
    }

    if let Some(ref mask) = g.mask {
        xml.write_func_iri(AId::Mask, mask.id(), opt);
    }

    write_group_filters(g, opt, xml);

    if g.opacity != Opacity::ONE {
        xml.write_svg_attribute(AId::Opacity, &g.opacity.get());
    }

    xml.write_transform(AId::Transform, g.transform, opt);

    write_group_blending(g, xml);
}

fn write_group_blending(g: &Group, xml: &mut XmlWriter) {
    if g.blend_mode != BlendMode::Normal || g.isolate {
        // For reasons unknown, `mix-blend-mode` and `isolation` must be written
        // as `style` attribute.
        let isolation = if g.isolate { "isolate" } else { "auto" };
        xml.write_attribute_fmt(
            AId::Style.to_str(),
            format_args!("mix-blend-mode:{};isolation:{}", g.blend_mode, isolation),
        );
    }
}

fn write_group_filters(g: &Group, opt: &WriteOptions, xml: &mut XmlWriter) {
    if !g.filters.is_empty() {
        let prefix = opt.id_prefix.as_deref().unwrap_or_default();
        let ids: Vec<_> = g
            .filters
            .iter()
            .map(|filter| format!("url(#{}{})", prefix, filter.id()))
            .collect();
        xml.write_svg_attribute(AId::Filter, &ids.join(" "));
    }
}

trait XmlWriterExt {
    fn start_svg_element(&mut self, id: EId);
    fn write_svg_attribute<V: Display + ?Sized>(&mut self, id: AId, value: &V);
    fn write_id_attribute(&mut self, id: &str, opt: &WriteOptions);
    fn write_color(&mut self, id: AId, color: Color);
    fn write_units(&mut self, id: AId, units: Units, def: Units);
    fn write_transform(&mut self, id: AId, units: Transform, opt: &WriteOptions);
    fn write_visibility(&mut self, value: bool);
    fn write_func_iri(&mut self, aid: AId, id: &str, opt: &WriteOptions);
    fn write_rect_attrs(&mut self, r: NonZeroRect);
    fn write_numbers(&mut self, aid: AId, list: &[f32]);
    fn write_image_data(&mut self, kind: &ImageKind);
    fn write_filter_input(&mut self, id: AId, input: &filter::Input);
    fn write_filter_primitive_attrs(&mut self, parent_rect: NonZeroRect, fe: &filter::Primitive);
    fn write_filter_transfer_function(&mut self, eid: EId, fe: &filter::TransferFunction);
}

impl XmlWriterExt for XmlWriter {
    #[inline(never)]
    fn start_svg_element(&mut self, id: EId) {
        self.start_element(id.to_str());
    }

    #[inline(never)]
    fn write_svg_attribute<V: Display + ?Sized>(&mut self, id: AId, value: &V) {
        self.write_attribute(id.to_str(), value);
    }

    #[inline(never)]
    fn write_id_attribute(&mut self, id: &str, opt: &WriteOptions) {
        debug_assert!(!id.is_empty());

        if let Some(ref prefix) = opt.id_prefix {
            let full_id = format!("{}{}", prefix, id);
            self.write_attribute("id", &full_id);
        } else {
            self.write_attribute("id", id);
        }
    }

    #[inline(never)]
    fn write_color(&mut self, id: AId, c: Color) {
        static CHARS: &[u8] = b"0123456789abcdef";

        #[inline]
        fn int2hex(n: u8) -> (u8, u8) {
            (CHARS[(n >> 4) as usize], CHARS[(n & 0xf) as usize])
        }

        let (r1, r2) = int2hex(c.red);
        let (g1, g2) = int2hex(c.green);
        let (b1, b2) = int2hex(c.blue);

        self.write_attribute_raw(id.to_str(), |buf| {
            buf.extend_from_slice(&[b'#', r1, r2, g1, g2, b1, b2]);
        });
    }

    fn write_units(&mut self, id: AId, units: Units, def: Units) {
        if units != def {
            self.write_attribute(id.to_str(), &units);
        }
    }

    fn write_transform(&mut self, id: AId, ts: Transform, opt: &WriteOptions) {
        if !ts.is_default() {
            self.write_attribute_raw(id.to_str(), |buf| {
                buf.extend_from_slice(b"matrix(");
                write_num(ts.sx, buf, opt.transforms_precision);
                buf.push(b' ');
                write_num(ts.ky, buf, opt.transforms_precision);
                buf.push(b' ');
                write_num(ts.kx, buf, opt.transforms_precision);
                buf.push(b' ');
                write_num(ts.sy, buf, opt.transforms_precision);
                buf.push(b' ');
                write_num(ts.tx, buf, opt.transforms_precision);
                buf.push(b' ');
                write_num(ts.ty, buf, opt.transforms_precision);
                buf.extend_from_slice(b")");
            });
        }
    }

    fn write_visibility(&mut self, value: bool) {
        if !value {
            self.write_attribute(AId::Visibility.to_str(), "hidden");
        }
    }

    fn write_func_iri(&mut self, aid: AId, id: &str, opt: &WriteOptions) {
        debug_assert!(!id.is_empty());
        let prefix = opt.id_prefix.as_deref().unwrap_or_default();
        self.write_attribute_fmt(aid.to_str(), format_args!("url(#{}{})", prefix, id));
    }

    fn write_rect_attrs(&mut self, r: NonZeroRect) {
        self.write_svg_attribute(AId::X, &r.x());
        self.write_svg_attribute(AId::Y, &r.y());
        self.write_svg_attribute(AId::Width, &r.width());
        self.write_svg_attribute(AId::Height, &r.height());
    }

    fn write_numbers(&mut self, aid: AId, list: &[f32]) {
        self.write_attribute_raw(aid.to_str(), |buf| {
            for n in list {
                buf.write_fmt(format_args!("{} ", n)).unwrap();
            }

            if !list.is_empty() {
                buf.pop();
            }
        });
    }

    fn write_filter_input(&mut self, id: AId, input: &filter::Input) {
        self.write_attribute(
            id.to_str(),
            match input {
                filter::Input::SourceGraphic => "SourceGraphic",
                filter::Input::SourceAlpha => "SourceAlpha",
                filter::Input::Reference(s) => s,
            },
        );
    }

    fn write_filter_primitive_attrs(&mut self, parent_rect: NonZeroRect, fe: &filter::Primitive) {
        if parent_rect.x() != fe.rect().x() {
            self.write_svg_attribute(AId::X, &fe.rect().x());
        }
        if parent_rect.y() != fe.rect().y() {
            self.write_svg_attribute(AId::Y, &fe.rect().y());
        }
        if parent_rect.width() != fe.rect().width() {
            self.write_svg_attribute(AId::Width, &fe.rect().width());
        }
        if parent_rect.height() != fe.rect().height() {
            self.write_svg_attribute(AId::Height, &fe.rect().height());
        }

        self.write_attribute(
            AId::ColorInterpolationFilters.to_str(),
            match fe.color_interpolation {
                filter::ColorInterpolation::SRGB => "sRGB",
                filter::ColorInterpolation::LinearRGB => "linearRGB",
            },
        );
    }

    fn write_filter_transfer_function(&mut self, eid: EId, fe: &filter::TransferFunction) {
        self.start_svg_element(eid);

        match fe {
            filter::TransferFunction::Identity => {
                self.write_svg_attribute(AId::Type, "identity");
            }
            filter::TransferFunction::Table(values) => {
                self.write_svg_attribute(AId::Type, "table");
                self.write_numbers(AId::TableValues, values);
            }
            filter::TransferFunction::Discrete(values) => {
                self.write_svg_attribute(AId::Type, "discrete");
                self.write_numbers(AId::TableValues, values);
            }
            filter::TransferFunction::Linear { slope, intercept } => {
                self.write_svg_attribute(AId::Type, "linear");
                self.write_svg_attribute(AId::Slope, &slope);
                self.write_svg_attribute(AId::Intercept, &intercept);
            }
            filter::TransferFunction::Gamma {
                amplitude,
                exponent,
                offset,
            } => write_gamma_transfer(self, *amplitude, *exponent, *offset),
        }

        self.end_element();
    }

    fn write_image_data(&mut self, kind: &ImageKind) {
        let svg_string;
        let (mime, data) = match kind {
            ImageKind::JPEG(data) => ("jpeg", data.as_slice()),
            ImageKind::PNG(data) => ("png", data.as_slice()),
            ImageKind::GIF(data) => ("gif", data.as_slice()),
            ImageKind::WEBP(data) => ("webp", data.as_slice()),
            ImageKind::SVG(tree) => {
                svg_string = tree.to_string(&WriteOptions::default());
                ("svg+xml", svg_string.as_bytes())
            }
        };

        self.write_attribute_raw("xlink:href", |buf| {
            buf.extend_from_slice(b"data:image/");
            buf.extend_from_slice(mime.as_bytes());
            buf.extend_from_slice(b";base64, ");

            let mut enc =
                base64::write::EncoderWriter::new(buf, &base64::engine::general_purpose::STANDARD);
            enc.write_all(data).unwrap();
            enc.finish().unwrap();
        });
    }
}

fn write_gamma_transfer(xml: &mut XmlWriter, amplitude: f32, exponent: f32, offset: f32) {
    xml.write_svg_attribute(AId::Type, "gamma");
    xml.write_svg_attribute(AId::Amplitude, &amplitude);
    xml.write_svg_attribute(AId::Exponent, &exponent);
    xml.write_svg_attribute(AId::Offset, &offset);
}

fn has_xlink(parent: &Group) -> bool {
    for node in &parent.children {
        match node {
            Node::Group(g) if group_has_xlink(g) => return true,
            Node::Image(_) => {
                return true;
            }
            Node::Text(text) => {
                if text
                    .chunks
                    .iter()
                    .any(|t| matches!(t.text_flow, TextFlow::Path(_)))
                {
                    return true;
                }
            }
            _ => {}
        }

        let mut present = false;
        node.subroots(|root| present |= has_xlink(root));
        if present {
            return true;
        }
    }

    false
}

fn group_has_xlink(group: &Group) -> bool {
    for filter in &group.filters {
        if filter
            .primitives
            .iter()
            .any(|p| matches!(p.kind, filter::Kind::Image(_)))
        {
            return true;
        }
    }
    if let Some(mask) = &group.mask {
        if has_xlink(mask.root()) {
            return true;
        }
        if let Some(sub_mask) = &mask.mask {
            if has_xlink(&sub_mask.root) {
                return true;
            }
        }
    }
    has_xlink(group)
}

fn write_base_grad(g: &BaseGradient, opt: &WriteOptions, xml: &mut XmlWriter) {
    xml.write_units(AId::GradientUnits, g.units, Units::ObjectBoundingBox);
    xml.write_transform(AId::GradientTransform, g.transform, opt);

    match g.spread_method {
        SpreadMethod::Pad => {}
        SpreadMethod::Reflect => xml.write_svg_attribute(AId::SpreadMethod, "reflect"),
        SpreadMethod::Repeat => xml.write_svg_attribute(AId::SpreadMethod, "repeat"),
    }

    for s in &g.stops {
        xml.start_svg_element(EId::Stop);
        xml.write_svg_attribute(AId::Offset, &s.offset.get());
        xml.write_color(AId::StopColor, s.color);
        if s.opacity != Opacity::ONE {
            xml.write_svg_attribute(AId::StopOpacity, &s.opacity.get());
        }

        xml.end_element();
    }
}

fn write_path(
    path: &Path,
    is_clip_path: bool,
    path_transform: Transform,
    clip_path: Option<&str>,
    opt: &WriteOptions,
    xml: &mut XmlWriter,
) {
    xml.start_svg_element(EId::Path);
    if !path.id.is_empty() {
        xml.write_id_attribute(&path.id, opt);
    }

    write_fill(&path.fill, is_clip_path, opt, xml);
    write_stroke(&path.stroke, opt, xml);

    xml.write_visibility(path.visible);

    if path.paint_order == PaintOrder::StrokeAndFill {
        xml.write_svg_attribute(AId::PaintOrder, "stroke");
    }

    write_shape_rendering(path.rendering_mode, xml);

    if let Some(id) = clip_path {
        xml.write_func_iri(AId::ClipPath, id, opt);
    }

    xml.write_transform(AId::Transform, path_transform, opt);

    write_path_data(path, opt, xml);
    xml.end_element();
}

fn write_path_data(path: &Path, opt: &WriteOptions, xml: &mut XmlWriter) {
    xml.write_attribute_raw("d", |buf| {
        for seg in path.data.segments() {
            write_path_segment(seg, buf, opt.coordinates_precision);
        }

        buf.pop();
    });
}

fn write_shape_rendering(rendering_mode: ShapeRendering, xml: &mut XmlWriter) {
    match rendering_mode {
        ShapeRendering::OptimizeSpeed => {
            xml.write_svg_attribute(AId::ShapeRendering, "optimizeSpeed");
        }
        ShapeRendering::CrispEdges => xml.write_svg_attribute(AId::ShapeRendering, "crispEdges"),
        ShapeRendering::GeometricPrecision => {}
    }
}

fn write_path_segment(segment: tiny_skia_path::PathSegment, buf: &mut Vec<u8>, precision: u8) {
    use tiny_skia_path::PathSegment;
    match segment {
        PathSegment::MoveTo(p) => {
            buf.extend_from_slice(b"M ");
            write_path_point(p, buf, precision);
        }
        PathSegment::LineTo(p) => {
            buf.extend_from_slice(b"L ");
            write_path_point(p, buf, precision);
        }
        PathSegment::QuadTo(p1, p) => {
            buf.extend_from_slice(b"Q ");
            write_path_point(p1, buf, precision);
            write_path_point(p, buf, precision);
        }
        PathSegment::CubicTo(p1, p2, p) => {
            buf.extend_from_slice(b"C ");
            write_path_point(p1, buf, precision);
            write_path_point(p2, buf, precision);
            write_path_point(p, buf, precision);
        }
        PathSegment::Close => buf.extend_from_slice(b"Z "),
    }
}

fn write_path_point(point: tiny_skia_path::Point, buf: &mut Vec<u8>, precision: u8) {
    write_num(point.x, buf, precision);
    buf.push(b' ');
    write_num(point.y, buf, precision);
    buf.push(b' ');
}

fn write_fill(fill: &Option<Fill>, is_clip_path: bool, opt: &WriteOptions, xml: &mut XmlWriter) {
    if let Some(fill) = fill {
        write_paint(AId::Fill, &fill.paint, opt, xml);

        if fill.opacity != Opacity::ONE {
            xml.write_svg_attribute(AId::FillOpacity, &fill.opacity.get());
        }

        if !fill.rule.is_default() {
            let name = if is_clip_path {
                AId::ClipRule
            } else {
                AId::FillRule
            };

            xml.write_svg_attribute(name, "evenodd");
        }
    } else {
        xml.write_svg_attribute(AId::Fill, "none");
    }
}

fn write_stroke(stroke: &Option<Stroke>, opt: &WriteOptions, xml: &mut XmlWriter) {
    if let Some(stroke) = stroke {
        write_paint(AId::Stroke, &stroke.paint, opt, xml);

        write_stroke_metrics(stroke, xml);

        match stroke.linecap {
            LineCap::Butt => {}
            LineCap::Round => xml.write_svg_attribute(AId::StrokeLinecap, "round"),
            LineCap::Square => xml.write_svg_attribute(AId::StrokeLinecap, "square"),
        }

        match stroke.linejoin {
            LineJoin::Miter => {}
            LineJoin::MiterClip => xml.write_svg_attribute(AId::StrokeLinejoin, "miter-clip"),
            LineJoin::Round => xml.write_svg_attribute(AId::StrokeLinejoin, "round"),
            LineJoin::Bevel => xml.write_svg_attribute(AId::StrokeLinejoin, "bevel"),
        }

        if let Some(ref array) = stroke.dasharray {
            xml.write_numbers(AId::StrokeDasharray, array);
        }
    } else {
        // Always set `stroke` to `none` to override the parent value.
        // In 99.9% of the cases it's redundant, but a group with `filter` with `StrokePaint`
        // will set `stroke`, which will interfere with children nodes.
        xml.write_svg_attribute(AId::Stroke, "none");
    }
}

fn write_stroke_metrics(stroke: &Stroke, xml: &mut XmlWriter) {
    if stroke.opacity != Opacity::ONE {
        xml.write_svg_attribute(AId::StrokeOpacity, &stroke.opacity.get());
    }
    if !stroke.dashoffset.approx_zero_ulps(4) {
        xml.write_svg_attribute(AId::StrokeDashoffset, &stroke.dashoffset);
    }
    if !stroke.miterlimit.is_default() {
        xml.write_svg_attribute(AId::StrokeMiterlimit, &stroke.miterlimit.get());
    }
    if stroke.width.get() != 1.0 {
        xml.write_svg_attribute(AId::StrokeWidth, &stroke.width.get());
    }
}

fn write_paint(aid: AId, paint: &Paint, opt: &WriteOptions, xml: &mut XmlWriter) {
    match paint {
        Paint::Color(c) => xml.write_color(aid, *c),
        Paint::LinearGradient(lg) => {
            xml.write_func_iri(aid, lg.id(), opt);
        }
        Paint::RadialGradient(rg) => {
            xml.write_func_iri(aid, rg.id(), opt);
        }
        Paint::Pattern(patt) => {
            xml.write_func_iri(aid, patt.id(), opt);
        }
    }
}

fn write_light_source(light: &filter::LightSource, xml: &mut XmlWriter) {
    match light {
        filter::LightSource::DistantLight(light) => {
            xml.start_svg_element(EId::FeDistantLight);
            xml.write_svg_attribute(AId::Azimuth, &light.azimuth);
            xml.write_svg_attribute(AId::Elevation, &light.elevation);
        }
        filter::LightSource::PointLight(light) => {
            xml.start_svg_element(EId::FePointLight);
            xml.write_svg_attribute(AId::X, &light.x);
            xml.write_svg_attribute(AId::Y, &light.y);
            xml.write_svg_attribute(AId::Z, &light.z);
        }
        filter::LightSource::SpotLight(light) => {
            xml.start_svg_element(EId::FeSpotLight);
            xml.write_svg_attribute(AId::X, &light.x);
            xml.write_svg_attribute(AId::Y, &light.y);
            xml.write_svg_attribute(AId::Z, &light.z);
            xml.write_svg_attribute(AId::PointsAtX, &light.points_at_x);
            xml.write_svg_attribute(AId::PointsAtY, &light.points_at_y);
            xml.write_svg_attribute(AId::PointsAtZ, &light.points_at_z);
            xml.write_svg_attribute(AId::SpecularExponent, &light.specular_exponent);
            if let Some(n) = light.limiting_cone_angle {
                xml.write_svg_attribute(AId::LimitingConeAngle, &n);
            }
        }
    }

    xml.end_element();
}

static POW_VEC: &[f32] = &[
    1.0,
    10.0,
    100.0,
    1_000.0,
    10_000.0,
    100_000.0,
    1_000_000.0,
    10_000_000.0,
    100_000_000.0,
    1_000_000_000.0,
    10_000_000_000.0,
    100_000_000_000.0,
    1_000_000_000_000.0,
];

fn write_num(num: f32, buf: &mut Vec<u8>, precision: u8) {
    // If number is an integer, it's faster to write it as i32.
    if num.fract().approx_zero_ulps(4) {
        write!(buf, "{}", num as i32).unwrap();
        return;
    }

    // Round numbers up to the specified precision to prevent writing
    // ugly numbers like 29.999999999999996.
    // It's not 100% correct, but differences are insignificant.
    //
    // Note that at least in Rust 1.64 the number formatting in debug and release modes
    // can be slightly different. So having a lower precision makes
    // our output and tests reproducible.
    let v = (num * POW_VEC[precision as usize]).round() / POW_VEC[precision as usize];

    write!(buf, "{}", v).unwrap();
}

/// Write all of the tspan attributes except for decorations.
fn write_span(
    is_clip_path: bool,
    opt: &WriteOptions,
    xml: &mut XmlWriter,
    chunk: &TextChunk,
    span: &TextSpan,
) {
    xml.start_svg_element(EId::Tspan);

    write_span_font(span, opt, xml);
    write_span_layout(span, xml);

    write_dominant_baseline(span, xml);
    write_alignment_baseline(span, xml);

    write_fill(&span.fill, is_clip_path, opt, xml);
    write_stroke(&span.stroke, opt, xml);
    write_span_content(chunk, span, xml);
    xml.end_element();
}

fn write_span_content(chunk: &TextChunk, span: &TextSpan, xml: &mut XmlWriter) {
    for baseline_shift in &span.baseline_shift {
        xml.start_svg_element(EId::Tspan);
        match baseline_shift {
            BaselineShift::Baseline => {}
            BaselineShift::Number(num) => xml.write_svg_attribute(AId::BaselineShift, num),
            BaselineShift::Subscript => xml.write_svg_attribute(AId::BaselineShift, "sub"),
            BaselineShift::Superscript => xml.write_svg_attribute(AId::BaselineShift, "super"),
        }
    }

    let cur_text = &chunk.text[span.start..span.end];

    xml.write_text(&cur_text.replace('&', "&amp;"));

    // End for each tspan we needed to create for baseline_shift
    for _ in &span.baseline_shift {
        xml.end_element();
    }
}

fn write_span_font(span: &TextSpan, opt: &WriteOptions, xml: &mut XmlWriter) {
    write_span_families(span, opt, xml);

    match span.font.style {
        FontStyle::Normal => {}
        FontStyle::Italic => xml.write_svg_attribute(AId::FontStyle, "italic"),
        FontStyle::Oblique => xml.write_svg_attribute(AId::FontStyle, "oblique"),
    }

    if span.font.weight != 400 {
        xml.write_svg_attribute(AId::FontWeight, &span.font.weight);
    }

    write_span_stretch(span, xml);
}

fn write_span_families(span: &TextSpan, opt: &WriteOptions, xml: &mut XmlWriter) {
    // WHY: SVGパーサーは空のfont-familyを既定のフォント名で補う。
    let families = span
        .font
        .families
        .iter()
        .map(|family| font_family_to_string(family, opt))
        .collect::<Vec<_>>()
        .join(", ");
    xml.write_svg_attribute(AId::FontFamily, &families);
}

fn font_family_to_string(font_family: &FontFamily, opt: &WriteOptions) -> String {
    match font_family {
        FontFamily::Monospace => "monospace".to_string(),
        FontFamily::Serif => "serif".to_string(),
        FontFamily::SansSerif => "sans-serif".to_string(),
        FontFamily::Cursive => "cursive".to_string(),
        FontFamily::Fantasy => "fantasy".to_string(),
        FontFamily::Named(s) => {
            // Only quote if absolutely necessary
            match parse_font_families(s) {
                Ok(_) => s.clone(),
                Err(_) => {
                    if opt.use_single_quote {
                        format!("\"{}\"", s)
                    } else {
                        format!("'{}'", s)
                    }
                }
            }
        }
    }
}

fn write_span_stretch(span: &TextSpan, xml: &mut XmlWriter) {
    let name = match span.font.stretch {
        FontStretch::Condensed => "condensed",
        FontStretch::ExtraCondensed => "extra-condensed",
        FontStretch::UltraCondensed => "ultra-condensed",
        FontStretch::SemiCondensed => "semi-condensed",
        FontStretch::Expanded => "expanded",
        FontStretch::SemiExpanded => "semi-expanded",
        FontStretch::ExtraExpanded => "extra-expanded",
        FontStretch::UltraExpanded => "ultra-expanded",
        FontStretch::Normal => return,
    };
    xml.write_svg_attribute(AId::FontStretch, name);
}

fn write_span_layout(span: &TextSpan, xml: &mut XmlWriter) {
    xml.write_svg_attribute(AId::FontSize, &span.font_size);

    xml.write_visibility(span.visible);
    write_span_spacing(span, xml);

    if span.small_caps {
        xml.write_svg_attribute(AId::FontVariant, "small-caps");
    }

    if span.paint_order == PaintOrder::StrokeAndFill {
        xml.write_svg_attribute(AId::PaintOrder, "stroke fill");
    }

    if !span.apply_kerning {
        xml.write_attribute("style", "font-kerning:none");
    }
}

fn write_span_spacing(span: &TextSpan, xml: &mut XmlWriter) {
    if span.letter_spacing != 0.0 {
        xml.write_svg_attribute(AId::LetterSpacing, &span.letter_spacing);
    }

    if span.word_spacing != 0.0 {
        xml.write_svg_attribute(AId::WordSpacing, &span.word_spacing);
    }

    if let Some(text_length) = span.text_length {
        xml.write_svg_attribute(AId::TextLength, &text_length);
    }

    if span.length_adjust == LengthAdjust::SpacingAndGlyphs {
        xml.write_svg_attribute(AId::LengthAdjust, "spacingAndGlyphs");
    }
}

fn write_dominant_baseline(span: &TextSpan, xml: &mut XmlWriter) {
    let name = match span.dominant_baseline {
        DominantBaseline::UseScript => "use-script",
        DominantBaseline::NoChange => "no-change",
        DominantBaseline::ResetSize => "reset-size",
        DominantBaseline::TextBeforeEdge => "text-before-edge",
        DominantBaseline::Middle => "middle",
        DominantBaseline::Central => "central",
        DominantBaseline::TextAfterEdge => "text-after-edge",
        DominantBaseline::Ideographic => "ideographic",
        DominantBaseline::Alphabetic => "alphabetic",
        DominantBaseline::Hanging => "hanging",
        DominantBaseline::Mathematical => "mathematical",
        DominantBaseline::Auto => return,
    };
    xml.write_svg_attribute(AId::DominantBaseline, name);
}

fn write_alignment_baseline(span: &TextSpan, xml: &mut XmlWriter) {
    let name = match span.alignment_baseline {
        AlignmentBaseline::Baseline => "baseline",
        AlignmentBaseline::BeforeEdge => "before-edge",
        AlignmentBaseline::TextBeforeEdge => "text-before-edge",
        AlignmentBaseline::Middle => "middle",
        AlignmentBaseline::Central => "central",
        AlignmentBaseline::AfterEdge => "after-edge",
        AlignmentBaseline::TextAfterEdge => "text-after-edge",
        AlignmentBaseline::Ideographic => "ideographic",
        AlignmentBaseline::Alphabetic => "alphabetic",
        AlignmentBaseline::Hanging => "hanging",
        AlignmentBaseline::Mathematical => "mathematical",
        AlignmentBaseline::Auto => return,
    };
    xml.write_svg_attribute(AId::AlignmentBaseline, name);
}

#[cfg(test)]
crate::svg_engine::owned_svg_model_tests!("../../../src/svg_engine_writer_dispatch_boundary_tests.rs");
