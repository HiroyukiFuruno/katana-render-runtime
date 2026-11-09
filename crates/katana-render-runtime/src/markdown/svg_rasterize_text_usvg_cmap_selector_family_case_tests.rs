use resvg::usvg::{
    self,
    fontdb::{Database, FaceInfo, ID, Language, Weight},
};
use std::sync::Arc;

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

#[test]
fn selector_uses_loaded_noto_and_returns_none_for_empty_database() -> Result<(), String> {
    let mut loaded = Database::new();
    let ids = loaded.load_font_source(usvg::fontdb::Source::Binary(Arc::new(FONT_BYTES.to_vec())));
    let [id] = ids.as_slice() else {
        return Err("expected one Noto face".to_owned());
    };
    let mut loaded = Arc::new(loaded);
    let font = real_svg_font(&loaded)?;
    assert_eq!(
        font.families(),
        &[usvg::FontFamily::Named("Noto Sans".into())]
    );
    let selector = super::html_font_selector();
    assert_eq!(selector(&font, &mut loaded), Some(*id));
    let mut empty = Arc::new(Database::new());
    assert_eq!(empty.faces().count(), 0);
    assert_eq!(
        usvg::FontResolver::default_font_selector()(&font, &mut empty),
        None
    );
    assert_eq!(selector(&font, &mut empty), None);
    Ok(())
}

fn real_svg_font(database: &Arc<Database>) -> Result<usvg::Font, String> {
    let options = usvg::Options {
        fontdb: Arc::clone(database),
        ..usvg::Options::default()
    };
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><text id="run" font-family="Noto Sans">A</text></svg>"#;
    let tree = usvg::Tree::from_str(source, &options).map_err(|error| error.to_string())?;
    let Some(usvg::Node::Text(text)) = tree.node_by_id("run") else {
        return Err("SVG text node missing".to_owned());
    };
    text.chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .map(|span| span.font().clone())
        .ok_or("SVG font missing".to_owned())
}

#[test]
fn case_distinct_aliases_preserve_exact_order_and_generic_fallback() -> Result<(), String> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let source_face = database
        .faces()
        .next()
        .cloned()
        .ok_or("bundled Noto Sans face missing")?;
    database.remove_face(source_face.id);

    let normal_id = database.push_face_info(face_as_alias(&source_face, "Foo", Weight::NORMAL));
    let bold_id = database.push_face_info(face_as_alias(&source_face, "FOO", Weight::BOLD));
    assert_ne!(normal_id, bold_id);

    let database = Arc::new(database);
    let selected = |families| selected_family(&database, families);

    /* WHY: 同じDB内の大小文字別faceで、正確な名前の優先を実SVGのglyph IDで固定する。 */
    assert_eq!(selected("FOO")?, bold_id);
    assert_eq!(selected("Foo")?, normal_id);
    assert_eq!(selected("foo")?, normal_id);
    assert_eq!(selected("Foo, FOO")?, normal_id);
    assert_eq!(selected("FOO, Foo")?, bold_id);
    assert_eq!(selected("foo, FOO")?, normal_id);

    /* WHY: 前方の未知名やgeneric名を飛ばして、後続family名へ進む順序を固定する。 */
    assert_eq!(selected("Missing, FOO, sans-serif")?, bold_id);
    assert_eq!(selected("Missing, serif, FOO")?, bold_id);
    Ok(())
}

fn face_as_alias(base: &FaceInfo, family: &str, weight: Weight) -> FaceInfo {
    let mut face = base.clone();
    face.id = ID::dummy();
    face.families = vec![(family.to_string(), Language::English_UnitedStates)];
    face.weight = weight;
    face
}

fn selected_family(database: &Arc<Database>, families: &str) -> Result<ID, String> {
    let mut options = usvg::Options {
        fontdb: Arc::clone(database),
        ..usvg::Options::default()
    };
    options.font_resolver.select_font = super::html_font_selector();
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><text id="run" x="0" y="24" font-family="{families}" font-size="16" font-weight="700">DB</text></svg>"#
    );
    let tree = usvg::Tree::from_str(&svg, &options).map_err(|error| error.to_string())?;
    let Some(usvg::Node::Text(text)) = tree.node_by_id("run") else {
        return Err("SVG text node missing".to_string());
    };
    let ids = text
        .layouted()
        .iter()
        .flat_map(|span| span.positioned_glyphs.iter())
        .map(|glyph| glyph.font)
        .collect::<Vec<_>>();
    if ids.is_empty() || ids.iter().any(|id| *id != ids[0]) {
        return Err(format!("unexpected glyph face IDs: {ids:?}"));
    }
    Ok(ids[0])
}
