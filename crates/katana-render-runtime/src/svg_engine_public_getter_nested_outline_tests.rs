use super::{upstream_writer_font_database, usvg};
use rustybuzz::ttf_parser;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const SVG_WITH_SPACE: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><text id='target' x='8' y='24' font-family='Yellowtail'>A A</text></svg>";

#[test]
fn nested_svg_forwards_the_outline_provider_for_an_outline_free_glyph() -> Result<(), String> {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        font_resolver: usvg::FontResolver {
            glyph_outline_fallback: Some(Box::new(move |database, id, glyph, _| {
                assert_eq!(
                    Some(glyph.0),
                    space_glyph_id(database, id),
                    "provider must receive the space glyph"
                );
                observed.fetch_add(1, Ordering::Relaxed);
                None
            })),
            ..usvg::FontResolver::default()
        },
        ..usvg::Options::default()
    };
    let parsed = usvg::Tree::from_data_nested(SVG_WITH_SPACE, &options)
        .map_err(|error| error.to_string())?;
    assert!(parsed.node_by_id("target").is_some());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    Ok(())
}

fn space_glyph_id(database: &usvg::fontdb::Database, id: usvg::fontdb::ID) -> Option<u32> {
    let glyph = database.with_face_data(id, |bytes, index| {
        ttf_parser::Face::parse(bytes, index).ok()?.glyph_index(' ')
    })??;
    Some(u32::from(glyph.0))
}
