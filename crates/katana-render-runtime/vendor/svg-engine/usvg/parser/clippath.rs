// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::str::FromStr;
use std::sync::Arc;

use super::converter;
use super::svgtree::{AId, EId, SvgNode};
use crate::svg_engine::usvg::{ClipPath, Group, NonEmptyString, NonZeroRect, Transform, Units};

pub(crate) fn convert(
    node: SvgNode,
    state: &converter::State,
    object_bbox: Option<NonZeroRect>,
    cache: &mut converter::Cache,
) -> Option<Arc<ClipPath>> {
    // A `clip-path` attribute must reference a `clipPath` element.
    if node.tag_name() != Some(EId::ClipPath) {
        return None;
    }

    // The whole clip path should be ignored when a transform is invalid.
    let transform = resolve_clip_path_transform(node, state)?;

    let units = clip_units(node);

    let cacheable = units == Units::UserSpaceOnUse;
    if let Some(clip) = cached_clip(node, cacheable, cache) {
        return Some(clip);
    }

    let transform = apply_clip_units(transform, units, object_bbox)?;
    let clip_path = resolve_linked_clip(node, state, object_bbox, cache)?;
    build_clip(node, state, cache, cacheable, transform, clip_path)
}

fn cached_clip(node: SvgNode, cacheable: bool, cache: &converter::Cache) -> Option<Arc<ClipPath>> {
    // objectBoundingBoxは各ノード固有のため共有しない。
    if cacheable {
        return cache.clip_paths.get(node.element_id()).cloned();
    }
    None
}

fn clip_units(node: SvgNode) -> Units {
    node.attribute(AId::ClipPathUnits)
        .unwrap_or(Units::UserSpaceOnUse)
}

fn apply_clip_units(mut transform: Transform, units: Units, object_bbox: Option<NonZeroRect>) -> Option<Transform> {
    if units == Units::ObjectBoundingBox {
        let object_bbox = match object_bbox {
            Some(v) => v,
            None => {
                log::warn!("Clipping of zero-sized shapes is not allowed.");
                return None;
            }
        };

        let ts = Transform::from_bbox(object_bbox);
        transform = transform.pre_concat(ts);
    }
    Some(transform)
}

fn resolve_linked_clip(
    node: SvgNode,
    state: &converter::State,
    object_bbox: Option<NonZeroRect>,
    cache: &mut converter::Cache,
) -> Option<Option<Arc<ClipPath>>> {
    // Resolve linked clip path.
    let mut clip_path = None;
    if let Some(link) = node.attribute::<SvgNode>(AId::ClipPath) {
        clip_path = convert(link, state, object_bbox, cache);

        // Linked `clipPath` must be valid.
        if clip_path.is_none() {
            return None;
        }
    }

    Some(clip_path)
}

fn build_clip(
    node: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
    cacheable: bool,
    transform: Transform,
    clip_path: Option<Arc<ClipPath>>,
) -> Option<Arc<ClipPath>> {
    let mut id = NonEmptyString::new(node.element_id().to_string())?;
    // Generate ID only when we're parsing `objectBoundingBox` clip for the second time.
    if !cacheable && cache.clip_paths.contains_key(id.get()) {
        id = cache.gen_clip_path_id();
    }
    let id_copy = id.get().to_string();

    let mut clip = ClipPath {
        id,
        transform,
        clip_path,
        root: Group::empty(),
    };

    let mut clip_state = state.clone();
    clip_state.parent_clip_path = Some(node);
    converter::convert_clip_path_elements(node, &clip_state, cache, &mut clip.root);
    finish_clip(clip, id_copy, cache)
}

fn finish_clip(mut clip: ClipPath, id: String, cache: &mut converter::Cache) -> Option<Arc<ClipPath>> {
    if clip.root.has_children() {
        clip.root.calculate_bounding_boxes();
        let clip = Arc::new(clip);
        cache.clip_paths.insert(id, clip.clone());
        Some(clip)
    } else {
        // A clip path without children is invalid.
        None
    }
}

fn resolve_clip_path_transform(node: SvgNode, state: &converter::State) -> Option<Transform> {
    // Do not use Node::attribute::<Transform>, because it will always
    // return a valid transform.

    let value: &str = match node.attribute(AId::Transform) {
        Some(v) => v,
        None => return Some(Transform::default()),
    };

    let ts = parse_clip_transform(value)?;
    if ts.is_valid() {
        Some(node.resolve_transform(AId::Transform, state))
    } else {
        None
    }
}

fn parse_clip_transform(value: &str) -> Option<Transform> {
    let ts = match svgtypes::Transform::from_str(value) {
        Ok(v) => v,
        Err(_) => {
            log::warn!("Failed to parse {} value: '{}'.", AId::Transform, value);
            return None;
        }
    };

    Some(Transform::from_row(
        ts.a as f32,
        ts.b as f32,
        ts.c as f32,
        ts.d as f32,
        ts.e as f32,
        ts.f as f32,
    ))
}
