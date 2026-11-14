// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use svgtypes::{Length, LengthUnit as Unit};

use super::svgtree::{AId, EId, SvgNode};
use super::{OptionLog, converter};
use crate::svg_engine::usvg::{
    Group, Mask, MaskType, Node, NonEmptyString, NonZeroRect, Transform, Units,
};

pub(crate) fn convert(
    node: SvgNode,
    state: &converter::State,
    object_bbox: Option<NonZeroRect>,
    cache: &mut converter::Cache,
) -> Option<Arc<Mask>> {
    if node.tag_name() != Some(EId::Mask) {
        return None;
    }
    let (units, content_units) = mask_units(node);
    // objectBoundingBoxは対象ごとに座標が変わるため共有しない。
    let cacheable = units == Units::UserSpaceOnUse && content_units == Units::UserSpaceOnUse;
    if cacheable {
        if let Some(mask) = cache.masks.get(node.element_id()) {
            return Some(mask.clone());
        }
    }
    let (rect, mask_all) = resolved_mask_rect(node, units, state, object_bbox)?;
    let id = mask_id(node, cacheable, cache)?;
    let id_copy = id.get().to_string();
    let mask = if mask_all {
        empty_mask(id, rect)
    } else {
        build_mask(node, state, object_bbox, cache, id, rect, content_units)?
    };
    let mask = Arc::new(mask);
    cache.masks.insert(id_copy, mask.clone());
    Some(mask)
}

fn mask_units(node: SvgNode) -> (Units, Units) {
    (
        node.attribute(AId::MaskUnits)
            .unwrap_or(Units::ObjectBoundingBox),
        node.attribute(AId::MaskContentUnits)
            .unwrap_or(Units::UserSpaceOnUse),
    )
}

fn mask_rect(node: SvgNode, units: Units, state: &converter::State) -> Option<NonZeroRect> {
    let rect = NonZeroRect::from_xywh(
        node.convert_length(AId::X, units, state, Length::new(-10.0, Unit::Percent)),
        node.convert_length(AId::Y, units, state, Length::new(-10.0, Unit::Percent)),
        node.convert_length(AId::Width, units, state, Length::new(120.0, Unit::Percent)),
        node.convert_length(AId::Height, units, state, Length::new(120.0, Unit::Percent)),
    );
    rect.log_none(|| log::warn!("Mask '{}' has an invalid size. Skipped.", node.element_id()))
}

fn resolved_mask_rect(
    node: SvgNode,
    units: Units,
    state: &converter::State,
    object_bbox: Option<NonZeroRect>,
) -> Option<(NonZeroRect, bool)> {
    let mut rect = mask_rect(node, units, state)?;
    let mut mask_all = false;
    if units == Units::ObjectBoundingBox {
        if let Some(bbox) = object_bbox {
            rect = rect.bbox_transform(bbox);
        } else {
            // When mask units are `objectBoundingBox` and bbox is zero-sized - the whole
            // element should be masked.
            // Technically an UB, but this is what Chrome and Firefox do.
            mask_all = true;
        }
    }
    Some((rect, mask_all))
}

fn mask_id(node: SvgNode, cacheable: bool, cache: &mut converter::Cache) -> Option<NonEmptyString> {
    let mut id = NonEmptyString::new(node.element_id().to_string())?;
    // Generate ID only when we're parsing `objectBoundingBox` mask for the second time.
    if !cacheable && cache.masks.contains_key(id.get()) {
        id = cache.gen_mask_id();
    }
    Some(id)
}

fn empty_mask(id: NonEmptyString, rect: NonZeroRect) -> Mask {
    Mask {
        id,
        rect,
        kind: MaskType::Luminance,
        mask: None,
        root: Group::empty(),
    }
}

fn linked_mask(
    node: SvgNode,
    state: &converter::State,
    object_bbox: Option<NonZeroRect>,
    cache: &mut converter::Cache,
) -> Option<Option<Arc<Mask>>> {
    let mut mask = None;
    if let Some(link) = node.attribute::<SvgNode>(AId::Mask) {
        mask = convert(link, state, object_bbox, cache);

        // Linked `mask` must be valid.
        if mask.is_none() {
            return None;
        }
    }

    Some(mask)
}

fn build_mask(
    node: SvgNode,
    state: &converter::State,
    object_bbox: Option<NonZeroRect>,
    cache: &mut converter::Cache,
    id: NonEmptyString,
    rect: NonZeroRect,
    content_units: Units,
) -> Option<Mask> {
    let mask = linked_mask(node, state, object_bbox, cache)?;
    let kind = if node.attribute(AId::MaskType) == Some("alpha") {
        MaskType::Alpha
    } else {
        MaskType::Luminance
    };

    let mut mask = Mask {
        id,
        rect,
        kind,
        mask,
        root: Group::empty(),
    };
    populate_mask_root(
        &mut mask.root,
        node,
        state,
        object_bbox,
        content_units,
        cache,
    )?;
    Some(mask)
}

fn mask_subroot(content_units: Units, object_bbox: Option<NonZeroRect>) -> Option<Option<Group>> {
    if content_units != Units::ObjectBoundingBox {
        return Some(None);
    }
    let object_bbox = match object_bbox {
        Some(v) => v,
        None => {
            log::warn!("Masking of zero-sized shapes is not allowed.");
            return None;
        }
    };
    // 子要素にもbbox変換が伝わるよう絶対変換も初期化する。
    let mut group = Group::empty();
    group.transform = Transform::from_bbox(object_bbox);
    group.abs_transform = group.transform;
    Some(Some(group))
}

fn populate_mask_root(
    root: &mut Group,
    node: SvgNode,
    state: &converter::State,
    object_bbox: Option<NonZeroRect>,
    content_units: Units,
    cache: &mut converter::Cache,
) -> Option<()> {
    let mut subroot = mask_subroot(content_units, object_bbox)?;
    {
        let real_root = subroot.as_mut().unwrap_or(&mut *root);
        converter::convert_children(node, state, cache, real_root);

        // A mask without children at this point is invalid.
        // Only masks with zero bbox and `objectBoundingBox` can be empty.
        if !real_root.has_children() {
            return None;
        }
    }

    if let Some(mut subroot) = subroot {
        subroot.calculate_bounding_boxes();
        root.children.push(Node::Group(Box::new(subroot)));
    }

    root.calculate_bounding_boxes();
    super::paint_server::update_paint_servers(root, Transform::default(), None, None, cache);

    Some(())
}
