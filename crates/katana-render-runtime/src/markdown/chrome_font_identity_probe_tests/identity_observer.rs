use super::{PhysicalFace, ResolvedFont, face_identity, physical_file};
use crate::markdown::chrome_font_identity_probe_tests::{ObserverFace, SelectedFont};
use resvg::usvg::fontdb::Database;
use std::path::PathBuf;

pub(in crate::markdown::chrome_font_identity_probe_tests) fn resolve_observer_face(
    selected: &SelectedFont,
    observer_faces: &[ObserverFace],
) -> Result<ResolvedFont, String> {
    let matches = observer_faces
        .iter()
        .filter(|face| face.post_script_name == selected.post_script_name)
        .collect::<Vec<_>>();
    let [observer] = matches.as_slice() else {
        return Err("Chrome retained font observation has no unique matching face".to_string());
    };
    let identity = verified_observer_identity(observer, selected)?;
    Ok(ResolvedFont {
        identity,
        method: "Chrome retained font observation",
        core_text_descriptor: None,
    })
}

fn verified_observer_identity(
    observer: &ObserverFace,
    selected: &SelectedFont,
) -> Result<PhysicalFace, String> {
    let path = verified_observer_file(observer)?;
    let mut database = Database::new();
    database
        .load_font_file(&path)
        .map_err(|_| "Chrome retained font observation file is not a usable font".to_string())?;
    let identity =
        selected_observer_face(&database, observer.face_index, &selected.post_script_name)?;
    if identity.path != path
        || identity.file_sha256 != observer.file_sha256
        || identity.face_index != observer.face_index
    {
        return Err("Chrome retained font observation physical face differs".to_string());
    }
    verified_observer_file(observer)
        .map_err(|_| "Chrome retained font observation changed before verification".to_string())?;
    Ok(identity)
}

fn selected_observer_face(
    database: &Database,
    face_index: u32,
    post_script_name: &str,
) -> Result<PhysicalFace, String> {
    let mut matches = database
        .faces()
        .filter(|face| face.index == face_index && face.post_script_name == post_script_name);
    let (Some(face), None) = (matches.next(), matches.next()) else {
        return Err(
            "Chrome retained font observation face does not match the selected font".to_string(),
        );
    };
    face_identity(database, face)
        .map_err(|_| "Chrome retained font observation physical face is invalid".to_string())
}

fn verified_observer_file(observer: &ObserverFace) -> Result<PathBuf, String> {
    if !observer.file_path.is_absolute() {
        return Err("Chrome retained font observation path is not canonical".to_string());
    }
    let (path, sha256, _) = physical_file(&observer.file_path)
        .map_err(|_| "Chrome retained font observation file is unavailable".to_string())?;
    if path != observer.file_path || sha256 != observer.file_sha256 {
        return Err("Chrome retained font observation physical file differs".to_string());
    }
    Ok(path)
}
