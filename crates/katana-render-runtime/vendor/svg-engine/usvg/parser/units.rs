// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use svgtypes::{Length, LengthUnit as Unit};

use super::converter;
use super::svgtree::{AId, SvgNode};
use crate::svg_engine::usvg::Units;

#[inline(never)]
pub(crate) fn convert_length(
    length: Length,
    node: SvgNode,
    aid: AId,
    object_units: Units,
    state: &converter::State,
) -> f32 {
    let dpi = state.opt.dpi;
    let n = length.number as f32;
    match length.unit {
        Unit::None | Unit::Px => n,
        Unit::Em => n * resolve_font_size(node, state),
        Unit::Ex => {
            {
                n * resolve_css_ex(node, state)
            }
        }
        Unit::In => n * dpi,
        Unit::Cm => n * dpi / 2.54,
        Unit::Mm => n * dpi / 25.4,
        Unit::Pt => n * dpi / 72.0,
        Unit::Pc => n * dpi / 6.0,
        Unit::Percent => {
            if object_units == Units::ObjectBoundingBox {
                n / 100.0
            } else {
                let view_box = state.view_box;

                match aid {
                    AId::Cx
                    | AId::Dx
                    | AId::Fx
                    | AId::MarkerWidth
                    | AId::RefX
                    | AId::Rx
                    | AId::Width
                    | AId::X
                    | AId::X1
                    | AId::X2 => convert_percent(length, view_box.width()),
                    AId::Cy
                    | AId::Dy
                    | AId::Fy
                    | AId::Height
                    | AId::MarkerHeight
                    | AId::RefY
                    | AId::Ry
                    | AId::Y
                    | AId::Y1
                    | AId::Y2 => convert_percent(length, view_box.height()),
                    _ => {
                        let mut vb_len = view_box.width().powi(2) + view_box.height().powi(2);
                        vb_len = (vb_len / 2.0).sqrt();
                        convert_percent(length, vb_len)
                    }
                }
            }
        }
    }
}

pub(crate) fn convert_user_length(
    length: Length,
    node: SvgNode,
    aid: AId,
    state: &converter::State,
) -> f32 {
    convert_length(length, node, aid, Units::UserSpaceOnUse, state)
}

#[inline(never)]
pub(crate) fn convert_list(node: SvgNode, aid: AId, state: &converter::State) -> Option<Vec<f32>> {
    if let Some(text) = node.attribute::<&str>(aid) {
        let mut num_list = Vec::new();
        for length in svgtypes::LengthListParser::from(text).flatten() {
            num_list.push(convert_user_length(length, node, aid, state));
        }

        Some(num_list)
    } else {
        None
    }
}

fn convert_percent(length: Length, base: f32) -> f32 {
    base * (length.number as f32) / 100.0
}

#[inline(never)]
pub(crate) fn resolve_font_size(node: SvgNode, state: &converter::State) -> f32 {
    let nodes: Vec<_> = node.ancestors().collect();
    let mut font_size = state.opt.font_size;
    for n in nodes.iter().rev().skip(1) {
        // skip Root
        if let Some(length) = n.try_attribute::<Length>(AId::FontSize) {
            let dpi = state.opt.dpi;
            let number = length.number as f32;
            font_size = match length.unit {
                Unit::None | Unit::Px => number,
                Unit::Em => number * font_size,
                Unit::Ex => {
                    {
                        number * resolve_parent_css_ex(*n, state, font_size)
                    }
                }
                Unit::In => number * dpi,
                Unit::Cm => number * dpi / 2.54,
                Unit::Mm => number * dpi / 25.4,
                Unit::Pt => number * dpi / 72.0,
                Unit::Pc => number * dpi / 6.0,
                Unit::Percent => {
                    // If `font-size` has percent units that it's value
                    // is relative to the parent node `font-size`.
                    number * font_size * 0.01
                }
            }
        } else if let Some(name) = n.attribute(AId::FontSize) {
            font_size = convert_named_font_size(name, font_size);
        }
    }

    font_size
}

fn resolve_css_ex(node: SvgNode, state: &converter::State) -> f32 {
    let font_size = resolve_font_size(node, state);
    resolve_css_ex_at_size(node, state, font_size)
}

fn resolve_parent_css_ex(node: SvgNode, state: &converter::State, font_size: f32) -> f32 {
    node.parent()
        .map(|parent| resolve_css_ex_at_size(parent, state, font_size))
        .unwrap_or(font_size / 2.0)
}

fn resolve_css_ex_at_size(node: SvgNode, state: &converter::State, font_size: f32) -> f32 {
    let font = super::text::convert_font(node, state);
    let mut fontdb = state.fontdb.borrow_mut();
    crate::svg_engine::usvg::text::layout::css_font_x_height(&font, &state.opt.font_resolver, &mut fontdb)
        .map(|ratio| ratio * font_size)
        .unwrap_or(font_size / 2.0)
}

fn convert_named_font_size(name: &str, parent_font_size: f32) -> f32 {
    let factor = match name {
        "xx-small" => -3,
        "x-small" => -2,
        "small" => -1,
        "medium" => 0,
        "large" => 1,
        "x-large" => 2,
        "xx-large" => 3,
        "smaller" => -1,
        "larger" => 1,
        _ => {
            log::warn!("Invalid 'font-size' value: '{}'.", name);
            0
        }
    };

    // 'On a computer screen a scaling factor of 1.2 is suggested between adjacent indexes.'
    parent_font_size * 1.2f32.powi(factor)
}
