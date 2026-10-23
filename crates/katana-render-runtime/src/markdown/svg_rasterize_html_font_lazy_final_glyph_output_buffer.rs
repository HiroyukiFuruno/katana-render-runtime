use super::super::super::{Capture, sources};
use super::{FinalGlyphError, FinalGlyphTree};

pub(in super::super) fn append_tree(
    tree: FinalGlyphTree,
    errors: Vec<FinalGlyphError>,
    owners: Vec<sources::ByteWeak>,
    dropped: usize,
) {
    super::super::super::ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(capture) = active.as_mut() else {
            return;
        };
        capture.final_glyph_dropped += dropped;
        if !append_tree_row(capture, tree) {
            capture.final_glyph_dropped += 1 + errors.len();
            return;
        }
        append_errors(capture, errors);
        capture.final_glyph_shared_byte_owners.extend(owners);
    });
}

fn append_tree_row(capture: &mut Capture, tree: FinalGlyphTree) -> bool {
    if capture.final_glyph_record_count >= super::super::MAX_FINAL_GLYPH_RECORDS {
        capture.final_glyph_dropped += tree.faces.len();
        return false;
    }
    let capacity = super::super::MAX_FINAL_GLYPH_RECORDS - capture.final_glyph_record_count - 1;
    let mut faces = tree.faces;
    capture.final_glyph_dropped += faces.len().saturating_sub(capacity);
    faces.truncate(capacity);
    capture.final_glyph_record_count += faces.len() + 1;
    capture
        .final_glyph_trees
        .push(FinalGlyphTree { faces, ..tree });
    true
}

fn append_errors(capture: &mut Capture, errors: Vec<FinalGlyphError>) {
    for error in errors {
        if capture.final_glyph_record_count >= super::super::MAX_FINAL_GLYPH_RECORDS {
            capture.final_glyph_dropped += 1;
        } else {
            capture.final_glyph_errors.push(error);
            capture.final_glyph_record_count += 1;
        }
    }
}
