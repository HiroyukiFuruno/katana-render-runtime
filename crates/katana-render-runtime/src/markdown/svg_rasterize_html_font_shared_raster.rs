use super::super::super::text_shaping::FileStamp;
use resvg::usvg::fontdb::{Database, FaceInfo, Family, ID, Source};
use std::{collections::BTreeMap, path::PathBuf};

#[path = "svg_rasterize_html_font_shared_sources.rs"]
pub(super) mod shared_sources;
use shared_sources::SharedSources;

type FreshDatabase = (Database, Vec<FaceInfo>);

pub(super) fn build(
    original: &Database,
    prefix: &[FaceInfo],
    stamps: &BTreeMap<PathBuf, FileStamp>,
) -> Option<FreshDatabase> {
    let mut database = Database::new();
    copy_generic_families(original, &mut database);
    let mut sources = SharedSources::default();
    let mut ids = Vec::new();
    for face in original.faces() {
        let mut fresh = face.clone();
        if prefix.iter().any(|allowed| allowed.id == face.id) {
            fresh.source = shared_source(&mut sources, stamps, &face.source)?;
        }
        fresh.id = ID::dummy();
        ids.push((face.id, database.push_face_info(fresh)));
    }
    let prefix = remap_prefix(&database, prefix, &ids)?;
    Some((database, prefix))
}

fn shared_source(
    sources: &mut SharedSources,
    stamps: &BTreeMap<PathBuf, FileStamp>,
    source: &Source,
) -> Option<Source> {
    match source {
        Source::File(path) => {
            let data = sources.get(path, stamps.get(path)?)?;
            Some(Source::SharedFile(path.clone(), data))
        }
        Source::Binary(_) => Some(source.clone()),
        Source::SharedFile(_, _) => None,
    }
}

fn remap_prefix(
    database: &Database,
    prefix: &[FaceInfo],
    ids: &[(ID, ID)],
) -> Option<Vec<FaceInfo>> {
    prefix
        .iter()
        .map(|face| {
            let (_, fresh) = ids.iter().find(|(old, _)| *old == face.id)?;
            database.face(*fresh).cloned()
        })
        .collect()
}

fn copy_generic_families(original: &Database, database: &mut Database) {
    database.set_serif_family(original.family_name(&Family::Serif).to_owned());
    database.set_sans_serif_family(original.family_name(&Family::SansSerif).to_owned());
    database.set_cursive_family(original.family_name(&Family::Cursive).to_owned());
    database.set_fantasy_family(original.family_name(&Family::Fantasy).to_owned());
    database.set_monospace_family(original.family_name(&Family::Monospace).to_owned());
}

#[cfg(test)]
#[path = "svg_rasterize_html_font_shared_raster_tests.rs"]
mod tests;
