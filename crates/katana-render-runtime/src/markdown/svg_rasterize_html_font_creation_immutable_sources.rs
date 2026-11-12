use super::super::shared_raster::shared_sources::{SharedBytes, SharedSourceSeed, SharedSources};
use super::CreationProof;
use resvg::usvg::fontdb::{Database, FaceInfo, ID, Source};
use std::{path::Path, sync::Arc};

impl CreationProof {
    pub(in super::super) fn load_shared(
        &mut self,
        database: &mut Database,
        paths: &[&str],
        sources: &mut SharedSources,
        remaining_bytes: &mut usize,
    ) {
        self.original_database.take();
        for path in paths {
            let sharing = (!self.failed).then_some((&mut *sources, &mut *remaining_bytes));
            super::source_loader::load_one(self, database, Path::new(path), sharing);
        }
    }

    pub(in super::super) fn selected_current(&self, database: &Database) -> bool {
        #[cfg(test)]
        super::super::raster_observer::count(
            super::super::raster_observer::Metric::ProofSelectedCurrent,
        );
        !self.failed
            && self.stamps.iter().all(|(path, stamp)| {
                self.immutable_shared.contains_key(path)
                    || super::super::super::super::text_shaping::observe_path_stamp(path).as_ref()
                        == Ok(stamp)
            })
            && self.prefix_current(database)
            && present_files_current_uncached(self)
            && self.absent_current()
    }

    pub(in super::super) fn contains(&self, id: ID) -> bool {
        self.prefix_face(id).is_some()
    }

    pub(super) fn prefix_face(&self, id: ID) -> Option<&FaceInfo> {
        self.prefix.iter().find(|face| face.id == id)
    }

    pub(in super::super) fn owns_immutable_source(&self, path: &Path, data: &SharedBytes) -> bool {
        self.immutable_shared
            .get(path)
            .is_some_and(|identity| *identity == Arc::as_ptr(data) as *const () as usize)
    }

    pub(in super::super) fn reusable_shared_sources(
        &self,
        database: &Database,
    ) -> Vec<SharedSourceSeed> {
        if !self.current(database) {
            return Vec::new();
        }
        self.immutable_shared
            .keys()
            .filter_map(|path| {
                let stamp = self.stamps.get(path)?;
                let data = database.faces().find_map(|face| match &face.source {
                    Source::SharedFile(source, data)
                        if source == path && self.owns_immutable_source(path, data) =>
                    {
                        Some(Arc::clone(data))
                    }
                    _ => None,
                })?;
                Some((path.clone(), stamp.clone(), data))
            })
            .collect()
    }
}

fn present_files_current_uncached(proof: &CreationProof) -> bool {
    proof.stamps.iter().all(|(path, stamp)| {
        proof.immutable_shared.contains_key(path)
            || stamp.durable_reusable() && {
                #[cfg(test)]
                super::super::raster_observer::count(
                    super::super::raster_observer::Metric::PositiveFreshEvaluation,
                );
                super::super::super::super::text_shaping::stamp_path_uncached(path).as_ref()
                    == Ok(stamp)
            }
    })
}
