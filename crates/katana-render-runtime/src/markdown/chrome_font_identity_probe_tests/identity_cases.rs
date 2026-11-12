use super::{ResourceFile, SelectedFont, identity::physical_file, test_support::TempDirectory};
use resvg::usvg::fontdb::FaceInfo;
use std::{fs, path::PathBuf};

#[path = "identity_cases_tests.rs"]
mod test_cases;

pub(super) use test_cases::{
    changed_custom_resource_bytes_are_rejected,
    missing_and_ambiguous_physical_faces_remain_unknown,
    observed_system_face_resolves_to_its_physical_file_and_index,
};

pub(super) fn selected_from_face(face: &FaceInfo) -> Result<SelectedFont, String> {
    let family_name = face
        .families
        .first()
        .map(|(name, _)| name.clone())
        .ok_or("system font face has no family name")?;
    Ok(SelectedFont {
        family_name,
        post_script_name: face.post_script_name.clone(),
        glyph_count: 1,
        is_custom_font: false,
    })
}

fn duplicate_face_resources(
    source_path: &PathBuf,
) -> Result<(TempDirectory, Vec<ResourceFile>), String> {
    let temp = TempDirectory::new()?;
    let file_name = source_path.file_name().ok_or("font path has no name")?;
    let first_path = temp.path().join(file_name);
    let second_path = temp
        .path()
        .join(format!("copy-{}", file_name.to_string_lossy()));
    for path in [&first_path, &second_path] {
        fs::copy(source_path, path).map_err(|error| error.to_string())?;
    }
    let (_, digest, _) = physical_file(&first_path)?;
    let resources = vec![
        resource("https://probe.invalid/font-a", first_path, digest.clone()),
        resource("https://probe.invalid/font-b", second_path, digest),
    ];
    Ok((temp, resources))
}

fn resource(url: &str, body_path: PathBuf, bytes_sha256: String) -> ResourceFile {
    ResourceFile {
        url: url.to_string(),
        body_path,
        bytes_sha256,
    }
}

fn append_changed_byte(path: &PathBuf) -> Result<(), String> {
    let mut bytes = fs::read(path).map_err(|error| error.to_string())?;
    bytes.push(0);
    fs::write(path, bytes).map_err(|error| error.to_string())
}
