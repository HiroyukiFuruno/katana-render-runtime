#[path = "svg_rasterize_html_font_creation_identity.rs"]
mod identity;
#[path = "svg_rasterize_html_font_creation_selected_shared_face.rs"]
mod selected_shared_face;
pub(super) use identity::same_face;
#[cfg(test)]
use identity::same_source;

use super::super::super::text_shaping::{FileStamp, observe_path_stamp, stamp_path_uncached};
use resvg::usvg::fontdb::{Database, FaceInfo, ID, Source};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, Weak},
};

#[derive(Default)]
pub(super) struct CreationProof {
    stamps: BTreeMap<PathBuf, FileStamp>,
    loaded_paths: std::collections::BTreeSet<PathBuf>,
    absent: Vec<PathBuf>,
    prefix: Vec<FaceInfo>,
    failed: bool,
    original_database: OnceLock<Weak<Database>>,
}

impl CreationProof {
    pub(super) fn load(&mut self, database: &mut Database, paths: &[&str]) {
        self.original_database.take();
        for path in paths {
            self.load_one(database, Path::new(path));
        }
    }

    fn load_one(&mut self, database: &mut Database, path: &Path) {
        if self.record_missing(path) {
            return;
        }
        let already_loaded = self.mark_loaded_identity(path);
        let before = stamp_path_uncached(path)
            .ok()
            .filter(FileStamp::durable_reusable);
        let load_failed = !already_loaded
            && database
                .load_font_source(Source::File(path.to_path_buf()))
                .is_empty();
        let after = stamp_path_uncached(path).ok();
        self.record_load(path, before, after, already_loaded, load_failed);
    }

    fn record_missing(&mut self, path: &Path) -> bool {
        if path.is_file() {
            return false;
        }
        match std::fs::metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.absent.push(path.to_path_buf())
            }
            _ => self.failed = true,
        }
        true
    }

    fn mark_loaded_identity(&mut self, path: &Path) -> bool {
        match super::sources::canonical_font_source_path(path) {
            Ok(identity) => !self.loaded_paths.insert(identity),
            Err(_) => {
                self.failed = true;
                false
            }
        }
    }

    fn record_load(
        &mut self,
        path: &Path,
        before: Option<FileStamp>,
        after: Option<FileStamp>,
        already_loaded: bool,
        load_failed: bool,
    ) {
        if before.is_none() || before != after || (!already_loaded && load_failed) {
            self.failed = true;
        }
        if let Some(stamp) = before {
            if self.stamps.get(path).is_some_and(|old| old != &stamp) {
                self.failed = true;
            }
            self.stamps.insert(path.to_path_buf(), stamp);
        }
    }

    pub(super) fn capture_prefix(&mut self, database: &Database) {
        self.original_database.take();
        self.prefix = database.faces().cloned().collect();
    }

    pub(super) fn current(&self, database: &Database) -> bool {
        #[cfg(test)]
        super::raster_observer::count(super::raster_observer::Metric::ProofCurrent);
        !self.failed
            && self.absent_current()
            && self
                .stamps
                .iter()
                .all(|(path, stamp)| observe_path_stamp(path).as_ref() == Ok(stamp))
            && self.prefix_current(database)
    }

    fn absent_current(&self) -> bool {
        self.absent.iter().all(|path| {
            #[cfg(test)]
            super::raster_observer::count(super::raster_observer::Metric::AbsentMetadata);
            matches!(std::fs::metadata(path),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound)
        })
    }

    pub(super) fn seal_original(&self, database: &Arc<Database>) -> bool {
        if !self.current(database) {
            return false;
        }
        /* WHY: Weak は DB を保持せず、safe Arc の COW・解放で元 allocation の証明が失効する。 */
        let _ = self.original_database.set(Arc::downgrade(database));
        true
    }

    fn original_matches(&self, database: &Database) -> bool {
        self.original_database
            .get()
            .and_then(Weak::upgrade)
            .is_some_and(|original| std::ptr::eq(original.as_ref(), database))
    }

    fn prefix_current(&self, database: &Database) -> bool {
        /* WHY: 検証済み同一 Arc の FaceInfo は不変。別 DB は従来通り全 prefix を比較する。 */
        self.original_matches(database)
            || self.prefix.iter().all(|face| {
                database
                    .face(face.id)
                    .is_some_and(|current| same_face(face, current))
            })
    }

    pub(super) fn transient(&self, database: &Arc<Database>) -> Option<(Arc<Database>, Arc<Self>)> {
        if !self.selected_current(database) {
            return None;
        }
        let (transient, prefix) =
            super::shared_raster::build(database, &self.prefix, &self.stamps)?;
        if !self.selected_current(database) {
            return None;
        }
        let proof = Self {
            stamps: self.stamps.clone(),
            loaded_paths: self.loaded_paths.clone(),
            absent: self.absent.clone(),
            prefix,
            failed: self.failed,
            original_database: OnceLock::new(),
        };
        let transient = Arc::new(transient);
        proof
            .current(&transient)
            .then(|| (transient, Arc::new(proof)))
    }

    fn present_files_current_uncached(&self) -> bool {
        self.stamps.iter().all(|(path, stamp)| {
            stamp.durable_reusable() && {
                #[cfg(test)]
                super::raster_observer::count(
                    super::raster_observer::Metric::PositiveFreshEvaluation,
                );
                stamp_path_uncached(path).as_ref() == Ok(stamp)
            }
        })
    }

    pub(super) fn selected_current(&self, database: &Database) -> bool {
        #[cfg(test)]
        super::raster_observer::count(super::raster_observer::Metric::ProofSelectedCurrent);
        /* WHY: 同一境界の不在パス走査だけをまとめ、世代の登録・直接検証は両方残す。 */
        !self.failed
            && self
                .stamps
                .iter()
                .all(|(path, stamp)| observe_path_stamp(path).as_ref() == Ok(stamp))
            && self.prefix_current(database)
            && self.present_files_current_uncached()
            && self.absent_current()
    }

    pub(super) fn contains(&self, id: ID) -> bool {
        self.prefix.iter().any(|face| face.id == id)
    }
}

#[cfg(test)]
#[path = "svg_rasterize_html_font_shared_proof_tests.rs"]
mod shared_tests;

#[cfg(test)]
#[path = "svg_rasterize_html_font_original_arc_proof_tests.rs"]
mod original_arc_tests;
