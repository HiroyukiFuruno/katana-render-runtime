use super::super::{PhysicalFace, face_identity, physical_file, supported_font_path};
use super::SelectedFont;
use resvg::usvg::fontdb::{Database, Source};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

type CandidateFacesByFile = BTreeMap<PathBuf, BTreeMap<u32, PhysicalFace>>;

pub(super) fn candidate_files(
    database: &Database,
    selected: &SelectedFont,
) -> Result<BTreeMap<PathBuf, PhysicalFace>, String> {
    let grouped = grouped_candidate_faces(database, selected)?;
    let mut candidates = BTreeMap::new();
    for (path, faces) in grouped {
        let identity = unique_candidate_face(faces)?;
        verify_single_font_file(&path, &identity)?;
        candidates.insert(path, identity);
    }
    if candidates.is_empty() {
        return Err(
            "selected family has no uniquely indexed physical font file in fontdb".to_string(),
        );
    }
    Ok(candidates)
}

fn grouped_candidate_faces(
    database: &Database,
    selected: &SelectedFont,
) -> Result<CandidateFacesByFile, String> {
    let mut grouped: CandidateFacesByFile = BTreeMap::new();
    for face in database.faces().filter(|face| {
        face.families
            .iter()
            .any(|(family, _)| family == &selected.family_name)
            && matches!(&face.source, Source::File(_) | Source::SharedFile(_, _))
    }) {
        insert_candidate_face(database, face, &mut grouped)?;
    }
    Ok(grouped)
}

fn insert_candidate_face(
    database: &Database,
    face: &resvg::usvg::fontdb::FaceInfo,
    grouped: &mut CandidateFacesByFile,
) -> Result<(), String> {
    let identity = face_identity(database, face)?;
    if supported_font_path(&identity.path) {
        insert_physical_identity(grouped, identity)?;
    }
    Ok(())
}

fn insert_physical_identity(
    grouped: &mut CandidateFacesByFile,
    identity: PhysicalFace,
) -> Result<(), String> {
    let faces = grouped.entry(identity.path.clone()).or_default();
    if faces
        .get(&identity.face_index)
        .is_some_and(|previous| previous.file_sha256 != identity.file_sha256)
    {
        return Err("font file changed while indexing physical candidates".to_string());
    }
    faces.insert(identity.face_index, identity);
    Ok(())
}

fn unique_candidate_face(faces: BTreeMap<u32, PhysicalFace>) -> Result<PhysicalFace, String> {
    if faces.len() != 1 {
        return Err(
            "selected family has ambiguous physical face indices in a font file".to_string(),
        );
    }
    let (face_index, identity) = faces
        .into_iter()
        .next()
        .ok_or_else(|| "selected family physical face disappeared".to_string())?;
    if face_index != 0 {
        return Err("CoreText URL descriptors cannot prove nonzero face index".into());
    }
    Ok(identity)
}

fn verify_single_font_file(path: &Path, identity: &PhysicalFace) -> Result<(), String> {
    let (physical_path, file_sha, bytes) = physical_file(path)?;
    if physical_path != identity.path || file_sha != identity.file_sha256 {
        return Err("physical candidate changed before single-font verification".to_string());
    }
    verify_single_font_data(&bytes, identity.face_index)
}

pub(super) fn verify_single_font_data(bytes: &[u8], face_index: u32) -> Result<(), String> {
    if face_index != 0 {
        return Err(
            "CoreText URL descriptors cannot prove a nonzero collection face index".to_string(),
        );
    }
    match read_fonts::FileRef::new(bytes)
        .map_err(|error| format!("parse CoreText candidate font file: {error}"))?
    {
        read_fonts::FileRef::Font(_) => Ok(()),
        read_fonts::FileRef::Collection(_) => {
            Err("CoreText URL descriptors expose no collection index".into())
        }
    }
}
