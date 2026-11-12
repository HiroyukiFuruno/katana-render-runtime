use super::*;
use crate::markdown::svg_rasterize::font::BUNDLED_SANS_SERIF_FONT;

type ContextInput = (Arc<Database>, Arc<CreationProof>, ID);
const BINARY_FIXTURE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" y="20" font-family="Noto Sans">AB</text></svg>"#;

fn binary_context_input(count: usize) -> Result<ContextInput, String> {
    let mut parsed = Database::new();
    parsed.load_font_data(BUNDLED_SANS_SERIF_FONT.to_vec());
    let face = parsed.faces().next().ok_or("bundled face missing")?.clone();
    let mut database = Database::new();
    for _ in 0..count {
        database.push_face_info(face.clone());
    }
    let id = database.faces().next().ok_or("context face missing")?.id;
    let mut proof = CreationProof::default();
    proof.capture_prefix(&database);
    Ok((Arc::new(database), Arc::new(proof), id))
}

#[test]
fn selected_binary_face_uses_original_id_without_mapping_alias() -> Result<(), String> {
    let (database, proof, id) = binary_context_input(1)?;
    let context =
        Context::new(Arc::clone(&database), proof, "AB".into()).ok_or("binary context missing")?;
    assert!(!context.finish(&database));
    let attempt = context.begin().ok_or("binary attempt missing")?;
    let mut parsed = Arc::new(database.as_ref().clone());
    assert_eq!(context.render_face(&mut parsed, id, None), Some(id));
    assert_eq!(parsed.len(), database.len());
    assert!(
        context
            .state
            .lock()
            .map_err(|_| "state poisoned")?
            .as_ref()
            .ok_or("state missing")?
            .aliases
            .is_empty()
    );
    assert!(context.finish(&parsed));
    drop(attempt);
    assert!(!context.finish(&parsed));
    assert_state_empty(&context)?;
    Ok(())
}

#[test]
fn selector_database_copy_on_write_invalidates_admission_but_keeps_real_glyphs()
-> Result<(), String> {
    let (database, proof, id) = binary_context_input(1)?;
    let context =
        Context::new(Arc::clone(&database), proof, "AB".into()).ok_or("binary context missing")?;
    let attempt = context.begin().ok_or("attempt missing")?;
    let valid = Arc::new(AtomicBool::new(true));
    let selected = Arc::new(AtomicBool::new(false));
    let mut options = mutating_selector_options(&database, id);
    context.install(&mut options, Arc::clone(&valid), Arc::clone(&selected));
    let tree =
        usvg::Tree::from_str(BINARY_FIXTURE_SVG, &options).map_err(|error| error.to_string())?;
    assert!(selected.load(Ordering::Relaxed));
    assert!(!valid.load(Ordering::Relaxed));
    assert_eq!(database.len(), 1);
    let Some(usvg::Node::Text(text)) = tree.node_by_id("run") else {
        return Err("selected text missing".into());
    };
    let glyphs: Vec<_> = text
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
        .collect();
    assert_eq!(glyphs.len(), 2);
    assert!(glyphs.iter().all(|glyph| glyph.font == id));
    drop(attempt);
    assert_state_empty(&context)?;
    Ok(())
}

fn mutating_selector_options(database: &Arc<Database>, id: ID) -> usvg::Options<'static> {
    let mut options = usvg::Options {
        fontdb: Arc::clone(database),
        ..usvg::Options::default()
    };
    options.font_resolver.select_font = Box::new(move |_, selected| {
        Arc::make_mut(selected).load_font_data(BUNDLED_SANS_SERIF_FONT.to_vec());
        Some(id)
    });
    options
}

#[path = "svg_rasterize_html_font_selected_face_poison_tests.rs"]
mod poison;

#[test]
fn selected_context_real_faces_admit_exact_capacity_and_reject_next() -> Result<(), String> {
    for count in [MAX_REGISTERED_FACES, MAX_REGISTERED_FACES + 1] {
        let (database, proof, _) = binary_context_input(count)?;
        assert_eq!(database.len(), count);
        let weak = Arc::downgrade(&database);
        let context = Context::new(database, proof, "AB".into());
        assert_eq!(context.is_some(), count == MAX_REGISTERED_FACES);
        if let Some(context) = context {
            let attempt = context.begin().ok_or("capacity attempt missing")?;
            assert!(context.finish(&context.original));
            drop(attempt);
            assert!(!context.finish(&context.original));
            assert_state_empty(&context)?;
            drop(context);
        }
        assert!(weak.upgrade().is_none());
    }
    Ok(())
}

#[test]
fn selected_attempt_drop_rejects_finish_and_releases_real_mapping() -> Result<(), String> {
    let fixture = Fixture::new()?;
    assert!(!fixture.context.finish(&fixture.context.original));
    let weak = SvgRasterizeOps::with_html_font_render_scope(|| finished_mapping(&fixture))?;
    assert!(weak.upgrade().is_none());
    assert!(!fixture.context.finish(&fixture.context.original));
    assert_state_empty(&fixture.context)?;
    Ok(())
}

fn finished_mapping(fixture: &Fixture) -> Result<fixture::WeakBytes, String> {
    let attempt = fixture.context.begin().ok_or("real attempt missing")?;
    let mut database = fixture.database();
    let alias = fixture.select(&mut database, fixture.a)?;
    let weak = fixture.assert_face(&database, fixture.a, alias)?;
    assert!(fixture.context.finish(&database));
    drop(database);
    assert!(weak.upgrade().is_some());
    drop(attempt);
    assert!(!fixture.context.finish(&fixture.context.original));
    assert!(weak.upgrade().is_none());
    Ok(weak)
}

type FaceSnapshot = Vec<resvg::usvg::fontdb::FaceInfo>;

fn file_capacity_context(fixture: &Fixture) -> Result<ContextInput, String> {
    let original = fixture
        .context
        .original
        .face(fixture.a)
        .ok_or("file face missing")?;
    let Source::File(path) = &original.source else {
        return Err("capacity fixture must use actual File".into());
    };
    let mut database = Database::new();
    let mut proof = CreationProof::default();
    proof.load(
        &mut database,
        &[path.to_str().ok_or("file path is not UTF-8")?],
    );
    assert_eq!(database.len(), 1);
    let face = database
        .faces()
        .next()
        .ok_or("loaded File face missing")?
        .clone();
    assert!(matches!(face.source, Source::File(_)));
    while database.len() < MAX_REGISTERED_FACES - 1 {
        database.push_face_info(face.clone());
    }
    let origin = database.faces().next().ok_or("capacity origin missing")?.id;
    proof.capture_prefix(&database);
    assert!(proof.current(&database));
    Ok((Arc::new(database), Arc::new(proof), origin))
}

fn assert_capacity_metadata(original: &Database, before: &FaceSnapshot, after: &Database) {
    use resvg::usvg::fontdb::Family;
    assert_eq!(before.len(), after.len());
    assert!(before.iter().zip(after.faces()).all(|(left, right)| {
        crate::markdown::svg_rasterize::font::html::proof::same_face(left, right)
    }));
    for family in [
        Family::Serif,
        Family::SansSerif,
        Family::Cursive,
        Family::Fantasy,
        Family::Monospace,
    ] {
        assert_eq!(original.family_name(&family), after.family_name(&family));
    }
}

fn assert_single_capacity_alias(
    context: &Context,
    database: &Arc<Database>,
    alias: ID,
) -> Result<(), String> {
    let locked = context.state.lock().map_err(|_| "state poisoned")?;
    let state = locked.as_ref().ok_or("capacity state missing")?;
    assert_eq!(state.aliases.len(), 1);
    let retained = state.aliases.first().ok_or("capacity alias missing")?;
    assert_eq!(retained.face.id, alias);
    assert!(retained.in_database(database));
    assert!(seal::database_current(
        &context.original,
        database,
        &state.aliases
    ));
    assert!(context.proof.current(&context.original));
    Ok(())
}

fn assert_capacity_shared_face(
    context: &Context,
    database: &Database,
    origin: ID,
    alias: ID,
) -> Result<fixture::WeakBytes, String> {
    let original = context
        .original
        .face(origin)
        .ok_or("capacity original missing")?;
    let actual = database
        .face(alias)
        .ok_or("capacity mapped alias missing")?;
    let Source::SharedFile(path, data) = &actual.source else {
        return Err("capacity append did not use SharedFile".into());
    };
    assert!(matches!(&original.source, Source::File(original_path) if path == original_path));
    let mut expected = original.clone();
    expected.id = alias;
    expected.source = actual.source.clone();
    assert!(crate::markdown::svg_rasterize::font::html::proof::same_face(&expected, actual));
    assert_ne!(origin, alias);
    Ok(Arc::downgrade(data))
}

fn reject_append_and_reuse(
    context: &Context,
    database: &mut Arc<Database>,
    origin: ID,
    alias: ID,
) -> Result<(), String> {
    let mut origins = context.original.faces();
    assert_eq!(origins.next().ok_or("first origin missing")?.id, origin);
    let next = origins.next().ok_or("next origin missing")?.id;
    let before: FaceSnapshot = database.faces().cloned().collect();
    let namespace = Arc::downgrade(database);
    assert_eq!(context.render_face(database, next, None), None);
    assert!(std::sync::Weak::ptr_eq(
        &namespace,
        &Arc::downgrade(database)
    ));
    assert_eq!(database.len(), MAX_REGISTERED_FACES);
    assert_capacity_metadata(&context.original, &before, database);
    assert_single_capacity_alias(context, database, alias)?;
    assert_eq!(context.render_face(database, origin, None), Some(alias));
    assert_capacity_metadata(&context.original, &before, database);
    assert_single_capacity_alias(context, database, alias)?;
    assert!(context.finish(database));
    Ok(())
}

fn capacity_mapping_lifetime(context: &Context, origin: ID) -> Result<(), String> {
    let attempt = context.begin().ok_or("capacity File attempt missing")?;
    let mut database = Arc::new(context.original.as_ref().clone());
    assert_eq!(database.len(), MAX_REGISTERED_FACES - 1);
    let prefix: FaceSnapshot = database.faces().cloned().collect();
    let alias = context
        .render_face(&mut database, origin, None)
        .ok_or("capacity append rejected")?;
    assert_eq!(database.len(), MAX_REGISTERED_FACES);
    assert!(prefix.iter().zip(database.faces()).all(|(left, right)| {
        crate::markdown::svg_rasterize::font::html::proof::same_face(left, right)
    }));
    let weak = assert_capacity_shared_face(context, &database, origin, alias)?;
    reject_append_and_reuse(context, &mut database, origin, alias)?;
    drop(database);
    assert!(weak.upgrade().is_some());
    drop(attempt);
    assert!(weak.upgrade().is_none());
    assert!(!context.finish(&context.original));
    assert_state_empty(context)?;
    Ok(())
}

#[test]
fn selected_file_alias_at_capacity_rejects_append_but_preserves_reuse_and_drop()
-> Result<(), String> {
    /* WHY: 実ファイルは検証scopeの終了まで保持し、fixture消去によるretryを混ぜない。 */
    let fixture = Fixture::new()?;
    let (database, proof, origin) = file_capacity_context(&fixture)?;
    let weak = Arc::downgrade(&database);
    let context =
        Context::new(database, proof, "AB".into()).ok_or("File capacity context missing")?;
    SvgRasterizeOps::with_html_font_render_scope(|| capacity_mapping_lifetime(&context, origin))?;
    assert_state_empty(&context)?;
    drop(context);
    assert!(weak.upgrade().is_none());
    Ok(())
}
