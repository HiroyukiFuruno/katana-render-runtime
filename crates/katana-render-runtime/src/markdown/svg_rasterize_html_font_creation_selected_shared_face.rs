use super::super::super::super::text_shaping::{observe_path_stamp, stamp_path_uncached};
use super::super::shared_raster::shared_sources::{SharedSources, validate_acquired_source};
use super::CreationProof;
use resvg::usvg::fontdb::{Database, FaceInfo, ID, Source};
use std::{path::PathBuf, sync::Arc};

impl CreationProof {
    fn selected_file_source(
        &self,
        path: &PathBuf,
        sources: &mut SharedSources,
        reused: Option<&Source>,
    ) -> Option<Source> {
        let stamp = self.stamps.get(path)?;
        let data = match reused {
            Some(Source::SharedFile(reused_path, data)) if reused_path == path => {
                /* WHY: 登録済み alias の library backing を使い、mapping 前後と同じ世代検証は残す。 */
                if !stamp.durable_reusable() || stamp_path_uncached(path).as_ref() != Ok(stamp) {
                    return None;
                }
                validate_acquired_source(path, stamp, Arc::clone(data))?
            }
            Some(_) => return None,
            None => sources.get(path, stamp)?,
        };
        Some(Source::SharedFile(path.clone(), data))
    }

    pub(in super::super) fn selected_shared_face(
        &self,
        database: &Database,
        id: ID,
        sources: &mut SharedSources,
        reused: Option<&Source>,
        admission: Option<&super::super::admission::Admission<'_>>,
    ) -> Option<FaceInfo> {
        /* WHY: 全 File は attempt 入口・出口で検証し、mapping は既存 get の path 前後検証で封印する。 */
        let prefix_face = self.prefix_face(id)?;
        if !self.selected_acquisition_current(database, id, admission) {
            return None;
        }
        /* WHY: prefixの照合はsourceの物理identityを含むので、確認済みのfaceをそのまま使える。 */
        let mut face = prefix_face.clone();
        match &face.source {
            Source::File(path) => {
                face.source = self.selected_file_source(path, sources, reused)?;
            }
            Source::SharedFile(path, data) if self.owns_immutable_source(path, data) => {}
            Source::Binary(_) => {}
            Source::SharedFile(_, _) => return None,
        }
        self.selected_acquisition_current(database, id, admission)
            .then_some(face)
    }

    fn selected_acquisition_current(
        &self,
        database: &Database,
        id: ID,
        admission: Option<&super::super::admission::Admission<'_>>,
    ) -> bool {
        let Some(admission) = admission else {
            return self.current(database);
        };
        /* WHY: 同一 admission の全不在走査は前後で完了し、公開前の finish でも再検証する。 */
        admission.matches(self, database, id)
            && !self.failed
            && self.stamps.iter().all(|(path, stamp)| {
                self.immutable_shared.contains_key(path)
                    || observe_path_stamp(path).as_ref() == Ok(stamp)
            })
            && self.prefix_current(database)
    }
}
