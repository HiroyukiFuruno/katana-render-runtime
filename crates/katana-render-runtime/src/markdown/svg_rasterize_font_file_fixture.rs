#[cfg(target_os = "macos")]
use resvg::usvg;
use resvg::usvg::fontdb::{Database, ID, Source};
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::sync::Arc;

pub(super) const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

pub(super) struct FontFileFixture {
    pub(super) database: Database,
    pub(super) face_id: ID,
    pub(super) path: PathBuf,
}

#[cfg(target_os = "macos")]
impl FontFileFixture {
    pub(super) fn parse_svg(&self, source: &str) -> Result<usvg::Tree, usvg::Error> {
        usvg::Tree::from_str(
            source,
            &usvg::Options {
                fontdb: Arc::new(self.database.clone()),
                ..usvg::Options::default()
            },
        )
    }
}

impl Drop for FontFileFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub(super) fn create(
    path: PathBuf,
    font_bytes: &[u8],
) -> Result<FontFileFixture, Box<dyn std::error::Error>> {
    std::fs::write(&path, font_bytes)?;
    let mut database = Database::new();
    database.load_font_source(Source::File(path.clone()));
    let face_id = database
        .faces()
        .next()
        .ok_or("fixture font did not load")?
        .id;
    Ok(FontFileFixture {
        database,
        face_id,
        path,
    })
}
