use super::{CreationProof, Database, HtmlFontRequest, REGULAR_TEXT, sources};
use resvg::usvg::fontdb::{ID, Language, Source, Stretch, Style, Weight};
use std::{path::PathBuf, sync::Arc};
use tempfile::TempDir;

pub(super) struct OwnedFontCopy {
    pub(super) directory: PathBuf,
    pub(super) path: PathBuf,
    _tempdir: TempDir,
}

impl OwnedFontCopy {
    pub(super) fn append_sentinel(&self, sentinel: u8) -> Result<(), String> {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .append(true)
            .open(&self.path)
            .and_then(|mut file| file.write_all(&[sentinel]))
            .map_err(|error| error.to_string())
    }

    pub(super) fn restore_len(&self, length: u64) -> Result<(), String> {
        std::fs::OpenOptions::new()
            .write(true)
            .open(&self.path)
            .and_then(|file| file.set_len(length))
            .map_err(|error| error.to_string())
    }

    pub(super) fn from_real_arial() -> Result<Self, String> {
        let request = HtmlFontRequest::from_text("Arial", REGULAR_TEXT);
        let (database, _) = sources::build_known_html_font_db(&request);
        let source = database
            .faces()
            .find_map(|face| {
                face.families
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case("Arial"))
                    .then(|| face.source.clone())
                    .and_then(|source| match source {
                        Source::File(path) => Some(path),
                        _ => None,
                    })
            })
            .ok_or_else(|| "no real Arial File face".to_string())?;
        let tempdir = tempfile::tempdir().map_err(|error| error.to_string())?;
        let directory = tempdir.path().to_path_buf();
        let copied = Self {
            path: directory.join("Arial.ttf"),
            directory,
            _tempdir: tempdir,
        };
        std::fs::copy(source, &copied.path).map_err(|error| error.to_string())?;
        Ok(copied)
    }
}

pub(super) fn copied_arial_database()
-> Result<(Arc<Database>, CreationProof, OwnedFontCopy), String> {
    let copied = OwnedFontCopy::from_real_arial()?;
    let path = copied
        .path
        .to_str()
        .ok_or_else(|| "owned Arial path is not UTF-8".to_string())?;
    let mut database = Database::new();
    let mut proof = CreationProof::default();
    proof.load(&mut database, &[path]);
    proof.capture_prefix(&database);
    Ok((Arc::new(database), proof, copied))
}

type FamilyNames = Vec<(String, Language)>;
pub(super) type FaceMetadata = (
    PathBuf,
    u32,
    FamilyNames,
    String,
    Style,
    Weight,
    Stretch,
    bool,
);

pub(super) fn face_metadata(database: &Database, id: ID) -> Result<FaceMetadata, String> {
    let face = database
        .face(id)
        .ok_or_else(|| "selected face disappeared".to_string())?;
    let Source::File(path) = &face.source else {
        return Err("selected face is not File-backed".to_string());
    };
    Ok((
        path.clone(),
        face.index,
        face.families.clone(),
        face.post_script_name.clone(),
        face.style,
        face.weight,
        face.stretch,
        face.monospaced,
    ))
}
