use crate::markdown::svg_rasterize::font::bundled_font_db;
use resvg::usvg::fontdb::{Database, Source};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

pub(super) struct TempFile(pub(super) PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn temporary_font_path(prefix: &str, counter: &AtomicUsize) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}.ttf",
        std::process::id(),
        counter.fetch_add(1, Ordering::Relaxed),
    ))
}

pub(super) struct CmapFileFixture {
    prefix: &'static str,
    counter: &'static AtomicUsize,
}

impl CmapFileFixture {
    pub(super) const fn new(prefix: &'static str, counter: &'static AtomicUsize) -> Self {
        Self { prefix, counter }
    }

    pub(super) fn bundled_database(
        &self,
    ) -> Result<(Arc<Database>, resvg::usvg::fontdb::ID, TempFile), String> {
        self.database(&bundled_bytes()?)
    }

    pub(super) fn database(
        &self,
        bytes: &[u8],
    ) -> Result<(Arc<Database>, resvg::usvg::fontdb::ID, TempFile), String> {
        let mut face = bundled_font_db()
            .faces()
            .next()
            .ok_or("bundled face missing")?
            .clone();
        let source = temporary_font_path(self.prefix, self.counter);
        std::fs::write(&source, bytes).map_err(|error| error.to_string())?;
        face.source = Source::File(source.clone());
        let mut database = Database::new();
        let id = database.push_face_info(face);
        Ok((Arc::new(database), id, TempFile(source)))
    }
}

pub(super) fn bundled_bytes() -> Result<Vec<u8>, String> {
    let database = bundled_font_db();
    let face = database.faces().next().ok_or("bundled face missing")?;
    database
        .with_face_data(face.id, |data, _| data.to_vec())
        .ok_or_else(|| "bundled bytes missing".to_string())
}
