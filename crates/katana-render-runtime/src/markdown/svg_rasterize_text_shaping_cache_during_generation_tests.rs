use super::{FONT_BYTES, cached_shaped_run, cached_shaped_run_width, font, key};
use resvg::usvg::fontdb::{Database, ID, Source};
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct ReplacedWhileReading {
    path: PathBuf,
    replacement: PathBuf,
    armed: AtomicBool,
}

impl AsRef<[u8]> for ReplacedWhileReading {
    fn as_ref(&self) -> &[u8] {
        if self.armed.swap(false, Ordering::Relaxed) {
            assert!(fs::rename(&self.replacement, &self.path).is_ok());
        }
        FONT_BYTES
    }
}

#[test]
fn shared_font_replaced_during_shaping_does_not_publish_stale_width()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let source = Arc::new(ReplacedWhileReading {
        path: directory.path().join("font.ttf"),
        replacement: directory.path().join("replacement.ttf"),
        armed: AtomicBool::new(false),
    });
    fs::write(&source.path, FONT_BYTES)?;
    fs::write(&source.replacement, [FONT_BYTES, &[0]].concat())?;
    let (database, id) = shared_database(&source)?;
    let text = super::FIRST_TEXT;
    source.armed.store(true, Ordering::Relaxed);
    assert_eq!(cached_shaped_run(&database, id, text, font(), None), None);
    assert!(!source.armed.load(Ordering::Relaxed));
    assert!(!source.replacement.exists());
    assert_eq!(cached_shaped_run_width(&database, &key(id, text)), None);
    let fresh = cached_shaped_run(&database, id, text, font(), None);
    assert!(fresh.is_some_and(|width| width.is_finite() && width > 0.0));
    assert_eq!(cached_shaped_run_width(&database, &key(id, text)), fresh);
    Ok(())
}

fn shared_database(
    source: &Arc<ReplacedWhileReading>,
) -> Result<(Arc<Database>, ID), &'static str> {
    let mut parsed = Database::new();
    parsed.load_font_data(FONT_BYTES.to_vec());
    let mut face = parsed
        .faces()
        .next()
        .ok_or("bundled font face missing")?
        .clone();
    let bytes = Arc::clone(source);
    face.source = Source::SharedFile(source.path.clone(), bytes);
    let mut shared = Database::new();
    shared.push_face_info(face);
    let id = shared.faces().next().ok_or("shared font face missing")?.id;
    Ok((Arc::new(shared), id))
}
