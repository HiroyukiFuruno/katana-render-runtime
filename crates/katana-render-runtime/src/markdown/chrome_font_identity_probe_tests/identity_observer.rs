use super::{PhysicalFace, ResolvedFont, physical_file};
use crate::markdown::chrome_font_identity_probe_tests::{ObserverFace, SelectedFont};
use rustybuzz::ttf_parser::Face;
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
    let identity = verified_observer_identity(observer)?;
    Ok(ResolvedFont {
        identity,
        method: "Chrome retained font observation",
        core_text_descriptor: None,
    })
}

fn verified_observer_identity(observer: &ObserverFace) -> Result<PhysicalFace, String> {
    let (path, bytes) = verified_observer_file(observer)?;
    /* WHY: 実描画時の名前はトレースで照合し、ここでは同じバイト列の面が読み取れることを再確認する。 */
    Face::parse(&bytes, observer.face_index)
        .map_err(|_| "Chrome retained font observation face index is invalid".to_string())?;
    verified_observer_file(observer)
        .map_err(|_| "Chrome retained font observation changed before verification".to_string())?;
    Ok(PhysicalFace {
        path,
        face_index: observer.face_index,
        file_sha256: observer.file_sha256.clone(),
    })
}

fn verified_observer_file(observer: &ObserverFace) -> Result<(PathBuf, Vec<u8>), String> {
    if !observer.file_path.is_absolute() {
        return Err("Chrome retained font observation path is not canonical".to_string());
    }
    let (path, sha256, bytes) = physical_file(&observer.file_path)
        .map_err(|_| "Chrome retained font observation file is unavailable".to_string())?;
    if path != observer.file_path || sha256 != observer.file_sha256 {
        return Err("Chrome retained font observation physical file differs".to_string());
    }
    Ok((path, bytes))
}
