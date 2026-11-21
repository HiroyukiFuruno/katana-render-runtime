use super::{matching_fallback_with_request, probe_font_has_char};
use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
use resvg::usvg::{
    self,
    fontdb::{Database, ID, Source},
};
use std::sync::Arc;

#[test]
fn requested_fallback_rejects_a_removed_base_face() -> Result<(), String> {
    let mut database = Arc::new(Database::new());
    let base = load_face(&mut database)?;
    let fallback = load_face(&mut database)?;
    assert_ne!(base, fallback, "separate font loads must have distinct IDs");
    let requested = requested_font(&database)?;
    assert_eq!(probe_font_has_char(&database, fallback, 'A'), Some(true));
    assert_eq!(
        requested_fallback(&database, base, &requested),
        (Some(fallback), true)
    );
    Arc::make_mut(&mut database).remove_face(base);

    assert_eq!(
        requested_fallback(&database, base, &requested),
        (None, false)
    );
    Ok(())
}

fn load_face(database: &mut Arc<Database>) -> Result<ID, String> {
    Arc::make_mut(database)
        .load_font_source(Source::Binary(Arc::new(FONT_BYTES)))
        .first()
        .copied()
        .ok_or("bundled face missing".to_owned())
}

fn requested_fallback(
    database: &Arc<Database>,
    base: ID,
    requested: &usvg::Font,
) -> (Option<ID>, bool) {
    matching_fallback_with_request(database, base, 'A', requested, |id, character| {
        probe_font_has_char(database, id, character)
    })
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
