use super::super::super::font::font_source_generation;
use super::super::fallback_generation::{
    FileGenerationDependencies, HtmlFallbackSelection, MAX_FILE_DEPENDENCIES,
};
use super::{HTML_FALLBACK_CACHE, HtmlFallbackKey, insert_cached, insert_html_face, lookup_cached};
use resvg::usvg;
use std::sync::Arc;

#[test]
fn fallback_cache_stops_recording_after_the_file_dependency_limit() -> Result<(), String> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/fonts/NotoSans-Regular.ttf"
    );
    let mut database = usvg::fontdb::Database::new();
    database
        .load_font_file(path)
        .map_err(|error| error.to_string())?;
    let id = database
        .faces()
        .next()
        .ok_or("bundled font face missing")?
        .id;
    let generation = font_source_generation(&database, id);
    assert!(generation.is_file());
    assert!(generation.durable_reusable());

    let mut dependencies = FileGenerationDependencies::new();
    for _ in 0..MAX_FILE_DEPENDENCIES {
        dependencies.observe(id, generation.clone(), generation.clone());
    }
    assert!(dependencies.reusable);
    dependencies.observe(id, generation.clone(), generation);
    assert!(!dependencies.reusable);
    assert_eq!(dependencies.entries.len(), MAX_FILE_DEPENDENCIES);
    Ok(())
}

#[test]
fn unavailable_generation_cannot_enter_or_reuse_durable_fallback_cache() -> Result<(), String> {
    let (database, face_id, generation) = unavailable_file_generation()?;
    let key = HtmlFallbackKey {
        base_face_id: face_id,
        character: 'A',
        requested_weight: 400,
        requested_italic: false,
    };
    let selection = HtmlFallbackSelection {
        face_id,
        dependencies: Arc::from([(face_id, generation)]),
    };
    seed_then_reject_lookup(&database, key, selection.clone());
    insert_cached(&database, key, selection);
    assert!(lookup_cached(&database, key).is_none());
    assert!(HTML_FALLBACK_CACHE.with(|cache| {
        cache
            .borrow()
            .iter()
            .all(|entry| !entry.faces.contains_key(&key))
    }));
    Ok(())
}

fn unavailable_file_generation() -> Result<
    (
        Arc<usvg::fontdb::Database>,
        usvg::fontdb::ID,
        super::super::fallback_generation::FontSourceGeneration,
    ),
    String,
> {
    let bundled = crate::markdown::svg_rasterize::font::bundled_font_db();
    let mut face = bundled
        .faces()
        .next()
        .ok_or("bundled font face missing")?
        .clone();
    face.source = usvg::fontdb::Source::File(std::env::temp_dir().join(format!(
        "krr-fallback-unavailable-{}.ttf",
        std::process::id()
    )));
    let mut database = usvg::fontdb::Database::new();
    let face_id = database.push_face_info(face);
    let database = Arc::new(database);
    let generation = font_source_generation(&database, face_id);
    if generation.durable_reusable() {
        return Err("missing file unexpectedly has a durable generation".into());
    }
    Ok((database, face_id, generation))
}

fn seed_then_reject_lookup(
    database: &Arc<usvg::fontdb::Database>,
    key: HtmlFallbackKey,
    selection: HtmlFallbackSelection,
) {
    HTML_FALLBACK_CACHE.with(|cache| {
        insert_html_face(&mut cache.borrow_mut(), database, key, selection);
    });
    assert!(lookup_cached(database, key).is_none());
}
