use super::super::super::SelectedFont;
use super::super::{PhysicalFace, face_identity, physical_file};
use resvg::usvg::fontdb::{Database, FaceInfo};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

pub(super) type IdentityMap = BTreeMap<(PathBuf, u32), PhysicalFace>;

pub(super) fn matching_identities(
    database: &Database,
    selected: &SelectedFont,
    allowed_paths: &BTreeSet<PathBuf>,
) -> Result<IdentityMap, String> {
    let mut identities = BTreeMap::new();
    for face in database
        .faces()
        .filter(|face| face.post_script_name == selected.post_script_name)
    {
        insert_matching_identity(&mut identities, database, face, selected, allowed_paths)?;
    }
    Ok(identities)
}

fn insert_matching_identity(
    identities: &mut IdentityMap,
    database: &Database,
    face: &FaceInfo,
    selected: &SelectedFont,
    allowed_paths: &BTreeSet<PathBuf>,
) -> Result<(), String> {
    let identity = face_identity(database, face)?;
    if selected.is_custom_font && !allowed_paths.contains(&identity.path) {
        return Ok(());
    }
    identities.insert((identity.path.clone(), identity.face_index), identity);
    Ok(())
}

pub(super) fn unique_identity(identities: IdentityMap) -> Result<Option<PhysicalFace>, String> {
    if identities.len() > 1 {
        return Err("multiple physical file/face-index matches are ambiguous".to_string());
    }
    Ok(identities.into_values().next())
}

pub(super) fn verify_physical_face(face: &PhysicalFace) -> Result<(), String> {
    let (path, digest, _) = physical_file(&face.path)?;
    if path != face.path || digest != face.file_sha256 {
        Err("physical font changed before the probe response was written".to_string())
    } else {
        Ok(())
    }
}
