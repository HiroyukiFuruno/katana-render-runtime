use super::path::{self, PreparedPath};
use resvg::usvg;
use std::{collections::HashMap, marker::PhantomData};

#[path = "svg_rasterize_composite_arc_plan_bounds.rs"]
mod bounds;
pub(super) use bounds::transform_bounds;

#[cfg(test)]
#[path = "svg_rasterize_composite_arc_plan_tests.rs"]
mod tests;

pub(super) struct NativeArcPlan<'a> {
    paths: HashMap<*const usvg::Path, PreparedPath>,
    groups: HashMap<*const usvg::Group, usvg::Rect>,
    tree: PhantomData<&'a usvg::Tree>,
}

impl<'a> NativeArcPlan<'a> {
    pub(super) fn build(tree: &'a usvg::Tree) -> Result<Self, String> {
        let mut builder = Builder::default();
        builder.visit(tree.root());
        let mut paths = HashMap::new();
        let mut groups = HashMap::new();
        paths
            .try_reserve(builder.paths.len())
            .map_err(|e| e.to_string())?;
        groups
            .try_reserve(builder.groups.len())
            .map_err(|e| e.to_string())?;
        paths.extend(builder.paths);
        groups.extend(builder.groups);
        Ok(Self {
            paths,
            groups,
            tree: PhantomData,
        })
    }

    pub(super) fn path(&self, path: &usvg::Path) -> Option<&PreparedPath> {
        self.paths.get(&(path as *const usvg::Path))
    }

    pub(super) fn group_bounds(&self, group: &usvg::Group) -> Option<usvg::Rect> {
        self.groups.get(&(group as *const usvg::Group)).copied()
    }
}

#[derive(Default)]
struct Builder {
    paths: Vec<(*const usvg::Path, PreparedPath)>,
    groups: Vec<(*const usvg::Group, usvg::Rect)>,
}

impl Builder {
    fn visit(&mut self, group: &usvg::Group) -> Option<usvg::Rect> {
        if !super::is_traversable_group(group) {
            return None;
        }
        let checkpoint = (self.paths.len(), self.groups.len());
        match self.visit_children(group) {
            Some(rect) => {
                self.groups.push((group as *const usvg::Group, rect));
                Some(rect)
            }
            None => {
                self.paths.truncate(checkpoint.0);
                self.groups.truncate(checkpoint.1);
                None
            }
        }
    }

    fn visit_children(&mut self, group: &usvg::Group) -> Option<usvg::Rect> {
        let mut combined = None;
        for node in group.children() {
            let rect = match node {
                usvg::Node::Path(node) => self.visit_path(node),
                usvg::Node::Group(child) => {
                    let checkpoint = (self.paths.len(), self.groups.len());
                    let Some(local) = self.visit(child) else {
                        continue;
                    };
                    match bounds::transform_bounds(local, child.transform()) {
                        Some(rect) => Some(rect),
                        None => {
                            self.paths.truncate(checkpoint.0);
                            self.groups.truncate(checkpoint.1);
                            None
                        }
                    }
                }
                _ => None,
            };
            if rect.is_some() {
                combined = bounds::union(combined, rect);
                combined?;
            }
        }
        combined
    }

    fn visit_path(&mut self, node: &usvg::Path) -> Option<usvg::Rect> {
        let prepared = path::prepare_native_arc(node)?;
        let rect = bounds::ink_bounds(&prepared)?;
        self.paths.push((node as *const usvg::Path, prepared));
        Some(rect)
    }
}
