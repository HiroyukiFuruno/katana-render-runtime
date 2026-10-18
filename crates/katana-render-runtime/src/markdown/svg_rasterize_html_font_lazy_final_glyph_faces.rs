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
mod tests {
    use super::*;
    use resvg::usvg::{Options, Tree, fontdb::Database};
    use std::{collections::BTreeMap, sync::Arc};

    #[test]
    fn unresolved_face_id_records_the_glyph_count_error() -> Result<(), String> {
        let tree = Tree::from_str(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#,
            &Options::default(),
        )
        .map_err(|error| error.to_string())?;
        let id = ID::dummy();
        let mut face_counts = BTreeMap::new();
        face_counts.insert(id, 7);

        let resolved = super::resolve(&tree, "root", 3, &face_counts);

        assert!(resolved.faces.is_empty());
        assert!(resolved.owners.is_empty());
        assert_eq!(resolved.errors.len(), 1);
        assert_eq!(resolved.errors[0].face_id, Some(id.to_string()));
        assert_eq!(resolved.errors[0].reason, "fontdb face ID is unresolved");
        Ok(())
    }

    #[test]
    fn binary_font_source_is_measure_only_and_never_physical_verified() -> Result<(), String> {
        let (tree, id) = binary_font_tree()?;
        let mut face_counts = BTreeMap::new();
        face_counts.insert(id, 2);

        let resolved = super::resolve(&tree, "root", 4, &face_counts);

        assert_eq!(resolved.faces.len(), 1);
        assert_eq!(resolved.faces[0].glyph_count, 2);
        assert_eq!(resolved.faces[0].identity, "unknown");
        assert!(resolved.faces[0].source.is_some());
        assert_eq!(resolved.owners.len(), 0);
        assert_eq!(resolved.errors.len(), 1);
        assert_eq!(
            resolved.errors[0].reason,
            "font source lacks stable physical file identity"
        );
        Ok(())
    }

    fn binary_font_tree() -> Result<(Tree, ID), String> {
        let mut database = Database::new();
        database.load_font_data(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf").to_vec());
        let face = database.faces().next().ok_or("binary font face missing")?;
        let id = face.id;
        let options = Options {
            fontdb: Arc::new(database),
            ..Options::default()
        };
        let tree = Tree::from_str(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#,
            &options,
        )
        .map_err(|error| error.to_string())?;
        Ok((tree, id))
    }
}
