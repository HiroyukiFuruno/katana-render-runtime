use super::super::marker_clip_mask_tests::MarkerClipMask;
use super::usvg;
use std::sync::Arc;

type ClipPair = (Arc<usvg::ClipPath>, Arc<usvg::ClipPath>);

/* WHY: この限定fixtureの共有clipと登録一覧を維持し、旧AAだけを比較する。 */
pub(super) fn change_marker_candidates(tree: &mut usvg::Tree) -> Result<usize, String> {
    if !tree.patterns.is_empty()
        || !tree.masks.is_empty()
        || !tree.filters.is_empty()
        || tree.root.clip_path.is_some()
    {
        return Err("unsupported paint server subroot".into());
    }
    let mut clips = Vec::new();
    let changed = change_group(&mut tree.root, &mut clips)?;
    replace_registry(&mut tree.clip_paths, &clips)?;
    let mut actual = Vec::new();
    tree.root.collect_clip_paths(&mut actual);
    if actual.len() != tree.clip_paths.len()
        || actual.iter().any(|item| {
            tree.clip_paths
                .iter()
                .filter(|entry| Arc::ptr_eq(entry, item))
                .count()
                != 1
        })
    {
        return Err("updated group and registry identities differ".into());
    }
    Ok(changed)
}

fn change_group(group: &mut usvg::Group, clips: &mut Vec<ClipPair>) -> Result<usize, String> {
    if group.mask.is_some() || !group.filters.is_empty() {
        return Err("unsupported group subroot".into());
    }
    let mut changed = usize::from(change_clip(&mut group.clip_path, clips)?);
    for node in &mut group.children {
        if let usvg::Node::Group(child) = node {
            changed += change_group(child, clips)?;
        } else {
            reject_subroot_clips(node)?;
        }
    }
    Ok(changed)
}

fn reject_subroot_clips(node: &usvg::Node) -> Result<(), String> {
    let mut found = false;
    node.subroots(|root| {
        let mut clips = Vec::new();
        root.collect_clip_paths(&mut clips);
        found |= root.clip_path.is_some() || !clips.is_empty();
    });
    if found {
        Err("unsupported clip in subroot".into())
    } else {
        Ok(())
    }
}

fn change_clip(
    slot: &mut Option<Arc<usvg::ClipPath>>,
    clips: &mut Vec<ClipPair>,
) -> Result<bool, String> {
    let Some(old) = slot else { return Ok(false) };
    if let Some((_, replacement)) = clips.iter().find(|(source, _)| Arc::ptr_eq(source, old)) {
        *slot = Some(Arc::clone(replacement));
        return Ok(true);
    }
    if clips.iter().any(|(source, _)| source.id() == old.id()) {
        return Err("distinct clips share an ID".into());
    }
    let replacement = clone_clip(old)?;
    clips.push((Arc::clone(old), Arc::clone(&replacement)));
    *slot = Some(replacement);
    Ok(true)
}

fn clone_clip(old: &usvg::ClipPath) -> Result<Arc<usvg::ClipPath>, String> {
    let [usvg::Node::Path(path)] = old.root.children.as_slice() else {
        return Err("unknown clip structure".into());
    };
    if path.rendering_mode != usvg::ShapeRendering::CrispEdges
        || !MarkerClipMask::is_rectangle(&path.data)
        || old.clip_path.is_some()
        || old.root.clip_path.is_some()
        || old.root.mask.is_some()
        || !old.root.filters.is_empty()
    {
        return Err("unknown clip or clip chain".into());
    }
    let mut root = old.root.clone();
    let [usvg::Node::Path(path)] = root.children.as_mut_slice() else {
        return Err("cloned clip path missing".into());
    };
    path.rendering_mode = usvg::ShapeRendering::GeometricPrecision;
    Ok(Arc::new(usvg::ClipPath {
        id: usvg::NonEmptyString::new(old.id.get().to_owned()).ok_or("cloned clip ID is empty")?,
        transform: old.transform,
        clip_path: None,
        root,
    }))
}

fn replace_registry(
    registry: &mut [Arc<usvg::ClipPath>],
    clips: &[ClipPair],
) -> Result<(), String> {
    if registry.len() != clips.len() {
        return Err("clip registry and group set differ".into());
    }
    for (old, new) in clips {
        let matching = registry
            .iter()
            .filter(|item| Arc::ptr_eq(item, old))
            .count();
        if matching != 1 || old.id() != new.id() {
            return Err("clip registry identity differs".into());
        }
    }
    for entry in registry {
        let Some((_, replacement)) = clips.iter().find(|(old, _)| Arc::ptr_eq(old, entry)) else {
            return Err("unmatched clip registry entry".into());
        };
        *entry = Arc::clone(replacement);
    }
    Ok(())
}
