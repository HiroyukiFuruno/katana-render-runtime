use super::super::{Context, seal};
use crate::markdown::svg_rasterize::{
    SvgRasterizeOps,
    font::html::{admission, proof::CreationProof},
};
use resvg::usvg::{
    self,
    fontdb::{Database, FaceInfo, ID, Source},
};
use std::{
    path::PathBuf,
    sync::{Arc, Weak},
};
use tempfile::TempDir;

type ProvenDatabase = (Arc<Database>, Arc<CreationProof>);
const REAL_FONT_PATH_COUNT: usize = 3;

pub(super) type WeakBytes = Weak<dyn AsRef<[u8]> + Send + Sync>;

pub(super) struct Fixture {
    pub(super) context: Arc<Context>,
    pub(super) a: ID,
    pub(super) b: ID,
    pub(super) ttc: ID,
    pub(super) root: PathBuf,
    _tempdir: TempDir,
}

impl Fixture {
    pub(super) fn new() -> Result<Self, String> {
        let tempdir = tempfile::tempdir().map_err(|error| error.to_string())?;
        Self::load(tempdir.path().to_path_buf(), tempdir)
    }

    fn load(root: PathBuf, tempdir: TempDir) -> Result<Self, String> {
        let (a, b, ttc) = write_real_fonts(&root)?;
        let (database, proof) = fixture_database(&root, [&a, &b, &ttc])?;
        let a_id = file_face(&database, &a, "Noto face missing")?;
        let b_id = file_face(&database, &b, "Arial face missing")?;
        let ttc_id = admitted_ttc_face(&database, &proof, &ttc)?;
        let context = Context::new(database, proof, "AB".into())
            .ok_or("fixture full durable proof rejected")?;
        Ok(Self {
            context,
            a: a_id,
            b: b_id,
            ttc: ttc_id,
            root,
            _tempdir: tempdir,
        })
    }

    pub(super) fn database(&self) -> Arc<Database> {
        Arc::new(self.context.original.as_ref().clone())
    }

    pub(super) fn select(&self, database: &mut Arc<Database>, original: ID) -> Result<ID, String> {
        let face = self
            .context
            .original
            .face(original)
            .ok_or("original face missing")?;
        assert_eq!(
            admitted(&self.context.original, &self.context.proof, face),
            Some(original)
        );
        self.context
            .render_face(database, original, None)
            .ok_or_else(|| "selected mapping rejected".to_owned())
    }

    fn original_face(&self, id: ID) -> Result<&FaceInfo, &'static str> {
        self.context
            .original
            .face(id)
            .ok_or("original metadata missing")
    }

    pub(super) fn assert_face(
        &self,
        database: &Arc<Database>,
        original: ID,
        alias: ID,
    ) -> Result<WeakBytes, String> {
        let original = self.original_face(original)?;
        let actual = database.face(alias).ok_or("alias missing")?;
        let Source::SharedFile(path, data) = &actual.source else {
            return Err("positive branch is not SharedFile".into());
        };
        assert!(matches!(&original.source, Source::File(original_path) if path == original_path));
        let mut expected = original.clone();
        expected.id = alias;
        expected.source = actual.source.clone();
        assert!(crate::markdown::svg_rasterize::font::html::proof::same_face(&expected, actual));
        assert!(seal::database_current(
            &self.context.original,
            database,
            &self
                .context
                .state
                .lock()
                .map_err(|_| "state poisoned")?
                .as_ref()
                .ok_or("attempt missing")?
                .aliases
        ));
        Ok(Arc::downgrade(data))
    }
}

fn admitted(database: &Arc<Database>, proof: &CreationProof, face: &FaceInfo) -> Option<ID> {
    let family = &face.families.first()?.0;
    admission::admit(
        database,
        proof,
        family,
        "AB",
        face.weight.0,
        face.style == usvg::fontdb::Style::Italic,
    )
}

fn write_real_fonts(root: &std::path::Path) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let a = root.join("noto.ttf");
    let b = root.join("arial.ttf");
    let ttc = root.join("helvetica.ttc");
    std::fs::write(
        &a,
        crate::markdown::svg_rasterize::font::BUNDLED_SANS_SERIF_FONT,
    )
    .map_err(|e| e.to_string())?;
    std::fs::copy("/System/Library/Fonts/Supplemental/Arial.ttf", &b).map_err(|e| e.to_string())?;
    std::fs::copy("/System/Library/Fonts/Helvetica.ttc", &ttc).map_err(|e| e.to_string())?;
    Ok((a, b, ttc))
}

fn fixture_database(
    root: &std::path::Path,
    paths: [&PathBuf; REAL_FONT_PATH_COUNT],
) -> Result<ProvenDatabase, String> {
    let absent = root.join("absent.ttf");
    let mut database = Database::new();
    let mut proof = CreationProof::default();
    for path in paths.into_iter().chain([&absent]) {
        proof.load(
            &mut database,
            &[path.to_str().ok_or("fixture path not UTF-8")?],
        );
    }
    for family in [
        usvg::fontdb::Family::Serif,
        usvg::fontdb::Family::SansSerif,
        usvg::fontdb::Family::Cursive,
        usvg::fontdb::Family::Fantasy,
        usvg::fontdb::Family::Monospace,
    ] {
        assert!(!database.family_name(&family).is_empty());
    }
    proof.capture_prefix(&database);
    Ok((Arc::new(database), Arc::new(proof)))
}

fn file_face(database: &Database, path: &std::path::Path, missing: &str) -> Result<ID, String> {
    database
        .faces()
        .find(|f| matches!(&f.source, Source::File(p) if p == path))
        .map(|face| face.id)
        .ok_or_else(|| missing.to_owned())
}

fn admitted_ttc_face(
    database: &Arc<Database>,
    proof: &CreationProof,
    ttc: &std::path::Path,
) -> Result<ID, String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        database
            .faces()
            .filter(|f| f.index > 0 && matches!(&f.source, Source::File(path) if path == ttc))
            .find(|face| admitted(database, proof, face) == Some(face.id))
            .map(|f| f.id)
    })
    .ok_or_else(|| "actual TTC nonzero face did not pass existing admission".to_owned())
}

#[cfg(test)]
#[path = "svg_rasterize_html_font_selected_face_fixture_tests.rs"]
mod tests;
