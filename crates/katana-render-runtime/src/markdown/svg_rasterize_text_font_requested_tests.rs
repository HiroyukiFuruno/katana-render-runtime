use super::matching_fallback_with_request;
use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
use resvg::usvg::{self, fontdb::Database};
use std::sync::Arc;

#[test]
fn requested_fallback_rejects_a_removed_base_face() -> Result<(), String> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let face = database.faces().next().ok_or("bundled face missing")?.id;
    let mut database = Arc::new(database);
    let requested = requested_font(&database)?;
    Arc::make_mut(&mut database).remove_face(face);

    assert_eq!(
        matching_fallback_with_request(&database, face, 'A', &requested, |_, _| Some(true)),
        (None, false)
    );
    Ok(())
}

fn requested_font(database: &Arc<Database>) -> Result<usvg::Font, String> {
    let options = usvg::Options {
        fontdb: Arc::clone(database),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><text id="text" font-family="Noto Sans">A</text></svg>"#,
        &options,
    )
    .map_err(|error| error.to_string())?;
    let Some(usvg::Node::Text(text)) = tree.node_by_id("text") else {
        return Err("SVG text node missing".to_owned());
    };
    text.chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .map(|span| span.font().clone())
        .ok_or("SVG font missing".to_owned())
}
