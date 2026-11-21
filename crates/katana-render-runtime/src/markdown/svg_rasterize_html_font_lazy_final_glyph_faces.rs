use super::super::{Mode, sources};
use super::output::{FinalGlyphError, FinalGlyphFace};
use resvg::usvg::{Tree, fontdb::ID};
use std::collections::BTreeMap;

pub(super) struct ResolvedFaces {
    pub(super) faces: Vec<FinalGlyphFace>,
    pub(super) errors: Vec<FinalGlyphError>,
    pub(super) owners: Vec<sources::ByteWeak>,
}

pub(super) fn resolve(
    tree: &Tree,
    tree_path: &str,
    paint_call: u64,
    face_counts: &BTreeMap<ID, u64>,
) -> ResolvedFaces {
    let mut resolved = ResolvedFaces {
        faces: Vec::new(),
        errors: Vec::new(),
        owners: Vec::new(),
    };
    for (&id, &glyph_count) in face_counts {
        resolve_one(tree, tree_path, paint_call, id, glyph_count, &mut resolved);
    }
    resolved
}

fn resolve_one(
    tree: &Tree,
    tree_path: &str,
    paint_call: u64,
    id: ID,
    glyph_count: u64,
    resolved: &mut ResolvedFaces,
) {
    let Some(face) = tree.fontdb().face(id) else {
        resolved.errors.push(face_error(
            paint_call,
            tree_path,
            id,
            "fontdb face ID is unresolved",
        ));
        return;
    };
    append_face_audit(tree_path, paint_call, id, glyph_count, face, resolved);
}

fn append_face_audit(
    tree_path: &str,
    paint_call: u64,
    id: ID,
    glyph_count: u64,
    face: &resvg::usvg::fontdb::FaceInfo,
    resolved: &mut ResolvedFaces,
) {
    let record = audited_face_record(face);
    if let Some(reason) = record.reason {
        resolved
            .errors
            .push(face_error(paint_call, tree_path, id, reason));
    }
    resolved.faces.push(FinalGlyphFace {
        face_id: id.to_string(),
        glyph_count,
        identity: record.identity,
        source: Some(record.source),
    });
    if let Some(owner) = record.owner {
        resolved.owners.push(owner);
    }
}

struct AuditedFaceRecord {
    source: sources::FaceSource,
    owner: Option<sources::ByteWeak>,
    identity: &'static str,
    reason: Option<&'static str>,
}

fn audited_face_record(face: &resvg::usvg::fontdb::FaceInfo) -> AuditedFaceRecord {
    if let Some((record, owner)) = sources::source_record(face, Mode::Audit) {
        return AuditedFaceRecord {
            source: record,
            owner,
            identity: "physical_verified",
            reason: None,
        };
    }
    let (record, owner) = sources::measured_source_record(face);
    AuditedFaceRecord {
        source: record,
        owner,
        identity: "unknown",
        reason: Some("font source lacks stable physical file identity"),
    }
}

fn face_error(paint_call: u64, tree_path: &str, id: ID, reason: &str) -> FinalGlyphError {
    FinalGlyphError {
        paint_call,
        tree_path: tree_path.into(),
        face_id: Some(id.to_string()),
        reason: reason.into(),
    }
}

#[cfg(test)]
#[path = "svg_rasterize_html_font_lazy_final_glyph_faces_tests.rs"]
mod tests;
