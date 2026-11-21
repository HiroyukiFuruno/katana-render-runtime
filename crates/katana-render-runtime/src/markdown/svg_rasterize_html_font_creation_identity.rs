use resvg::usvg::fontdb::{FaceInfo, Source};
use std::sync::Arc;

pub(in super::super) fn same_face(left: &FaceInfo, right: &FaceInfo) -> bool {
    left.id == right.id
        && left.index == right.index
        && left.families == right.families
        && left.post_script_name == right.post_script_name
        && left.style == right.style
        && left.weight == right.weight
        && left.stretch == right.stretch
        && left.monospaced == right.monospaced
        && same_source(&left.source, &right.source)
}

pub(super) fn same_source(left: &Source, right: &Source) -> bool {
    match (left, right) {
        (Source::File(left), Source::File(right)) => left == right,
        (Source::Binary(left), Source::Binary(right)) => Arc::ptr_eq(left, right),
        (Source::SharedFile(left_path, left), Source::SharedFile(right_path, right)) => {
            left_path == right_path && Arc::ptr_eq(left, right)
        }
        _ => false,
    }
}
