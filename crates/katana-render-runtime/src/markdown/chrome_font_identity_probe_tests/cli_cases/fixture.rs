use super::super::{
    ProbeRequest, ProbeResponse, ResourceFile, SelectedFont,
    identity::unique_file_face,
    test_support::{TempDirectory, run_probe_child},
};
use resvg::usvg::fontdb::{Database, FaceInfo};
use std::{fs, path::Path};

pub(super) struct CliFixture {
    pub(super) source_path: std::path::PathBuf,
    pub(super) face: FaceInfo,
    pub(super) family_name: String,
    pub(super) temp: TempDirectory,
    pub(super) nonfont_resource: ResourceFile,
}

impl CliFixture {
    pub(super) fn new() -> Result<Self, String> {
        let mut database = Database::new();
        database.load_system_fonts();
        let (source_path, face) =
            unique_file_face(&database).ok_or("no unique physical system face")?;
        let family_name = face
            .families
            .first()
            .map(|(name, _)| name.clone())
            .ok_or("system font face has no family")?;
        let temp = TempDirectory::new()?;
        let nonfont_resource = write_nonfont_resource(temp.path())?;
        Ok(Self {
            source_path,
            face,
            family_name,
            temp,
            nonfont_resource,
        })
    }

    pub(super) fn run_request(
        &self,
        response_name: &str,
        request: &ProbeRequest,
    ) -> Result<ProbeResponse, String> {
        let response_path = self.temp.path().join(response_name);
        run_probe_child(request, &response_path, super::PROBE_TEST)
    }
}

fn write_nonfont_resource(directory: &Path) -> Result<ResourceFile, String> {
    let bytes = b"captured image response body";
    let digest = super::super::identity::sha256(bytes);
    let path = directory.join(format!("1-{digest}.body"));
    fs::write(&path, bytes).map_err(|error| format!("write nonfont response body: {error}"))?;
    Ok(ResourceFile {
        url: "https://font-probe.invalid/image.png".to_string(),
        body_path: path,
        bytes_sha256: digest,
    })
}

pub(super) fn selected_font(
    family_name: String,
    post_script_name: String,
    glyph_count: u64,
    is_custom_font: bool,
) -> SelectedFont {
    SelectedFont {
        family_name,
        post_script_name,
        glyph_count,
        is_custom_font,
    }
}
