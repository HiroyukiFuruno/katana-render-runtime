use super::{CoreTextDescriptorEvidence, ResourceFile};
use resvg::usvg::fontdb::{Database, FaceInfo, Source};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

#[path = "identity_resources.rs"]
mod resources;

#[path = "identity_resolution.rs"]
mod resolution;

pub(super) use resolution::{FontResolutionCache, resolve_font_with_cache};

#[cfg(target_os = "macos")]
#[path = "identity_coretext.rs"]
mod coretext;

#[cfg(target_os = "macos")]
pub(super) fn verify_coretext_single_font_data_for_test(
    bytes: &[u8],
    face_index: u32,
) -> Result<(), String> {
    coretext::verify_single_font_data(bytes, face_index)
}

pub(super) fn load_resource_database(resources: &[ResourceFile]) -> Result<Database, String> {
    self::resources::load_resource_database(resources)
}

const SHA256_HEX_LENGTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct PhysicalFace {
    pub(super) path: PathBuf,
    pub(super) face_index: u32,
    pub(super) file_sha256: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ResolvedFont {
    pub(super) identity: PhysicalFace,
    pub(super) method: &'static str,
    pub(super) core_text_descriptor: Option<CoreTextDescriptorEvidence>,
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(SHA256_HEX_LENGTH);
    for byte in Sha256::digest(bytes) {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

pub(super) fn supported_font_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "ttf" | "otf" | "ttc" | "otc"
            )
        })
}

pub(super) fn physical_file(path: &Path) -> Result<(PathBuf, String, Vec<u8>), String> {
    let canonical =
        fs::canonicalize(path).map_err(|error| format!("canonicalize font: {error}"))?;
    let metadata = fs::metadata(&canonical).map_err(|error| format!("stat font: {error}"))?;
    if !metadata.is_file() {
        return Err("font source is not a regular file".to_string());
    }
    let bytes = fs::read(&canonical).map_err(|error| format!("read font: {error}"))?;
    let after = fs::metadata(&canonical).map_err(|error| format!("restat font: {error}"))?;
    if metadata.len() != after.len() || metadata.modified().ok() != after.modified().ok() {
        return Err("font file changed while hashing".to_string());
    }
    let digest = sha256(&bytes);
    Ok((canonical, digest, bytes))
}

fn face_path(face: &FaceInfo) -> Result<PathBuf, String> {
    match &face.source {
        Source::File(path) => Ok(path.clone()),
        Source::SharedFile(path, data) => {
            let (_, _, disk_bytes) = physical_file(path)?;
            if data.as_ref().as_ref() != disk_bytes.as_slice() {
                return Err("shared font bytes differ from the physical file".to_string());
            }
            Ok(path.clone())
        }
        Source::Binary(_) => {
            Err("font face is memory-only, not physically attributable".to_string())
        }
    }
}

pub(super) fn face_identity(database: &Database, face: &FaceInfo) -> Result<PhysicalFace, String> {
    let source_path = face_path(face)?;
    let (path, file_sha256, disk_bytes) = physical_file(&source_path)?;
    let backing_sha = database
        .with_face_data(face.id, |data, _| sha256(data))
        .ok_or_else(|| "fontdb face bytes could not be read".to_string())?;
    if backing_sha != file_sha256 || sha256(&disk_bytes) != file_sha256 {
        return Err("fontdb face bytes differ from the physical file".to_string());
    }
    Ok(PhysicalFace {
        path,
        face_index: face.index,
        file_sha256,
    })
}

pub(super) fn unique_file_face(database: &Database) -> Option<(PathBuf, FaceInfo)> {
    let mut by_postscript = BTreeMap::<String, Vec<FaceInfo>>::new();
    for face in database.faces() {
        if face.post_script_name.is_empty()
            || !matches!(&face.source, Source::File(_) | Source::SharedFile(_, _))
        {
            continue;
        }
        by_postscript
            .entry(face.post_script_name.clone())
            .or_default()
            .push(face.clone());
    }
    by_postscript.into_values().find_map(|faces| {
        let [face] = faces.as_slice() else {
            return None;
        };
        let path = face_path(face).ok()?;
        supported_font_path(&path).then_some((path, face.clone()))
    })
}

#[cfg(test)]
#[path = "identity_contract_tests.rs"]
mod contract_tests;
