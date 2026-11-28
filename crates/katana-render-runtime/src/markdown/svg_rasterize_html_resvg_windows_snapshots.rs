use super::super::{font, text_shaping};
use resvg::usvg::{
    self,
    fontdb::{Database, ID, Source},
};
use std::sync::Arc;

pub(super) fn parse(
    compatible_svg: &str,
    mut options: usvg::Options<'static>,
) -> Result<usvg::Tree, usvg::Error> {
    let original = Arc::clone(&options.fontdb);
    text_shaping::with_validated_tree_parse(|| {
        options.fontdb =
            snapshot_database(&original).map_or_else(|| Arc::clone(&original), Arc::new);
        usvg::Tree::from_str(compatible_svg, &options)
    })
}

fn snapshot_database(original: &Database) -> Option<Database> {
    text_shaping::enable_windows_font_snapshots();
    let mut snapshot = Database::new();
    font::copy_generic_families(original, &mut snapshot);
    let mut replaced = false;
    for face in original.faces() {
        let mut fresh = face.clone();
        if let Source::File(path) = &face.source
            && let Some(bytes) = text_shaping::windows_font_snapshot(path)
        {
            let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = bytes;
            fresh.source = Source::SharedFile(path.clone(), bytes);
            replaced = true;
        }
        fresh.id = ID::dummy();
        snapshot.push_face_info(fresh);
    }
    replaced.then_some(snapshot)
}

#[cfg(test)]
#[path = "svg_rasterize_html_resvg_windows_snapshot_tests.rs"]
mod tests;
