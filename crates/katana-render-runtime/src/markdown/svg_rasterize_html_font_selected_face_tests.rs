use super::*;
use crate::markdown::svg_rasterize::{SvgRasterizeOps, font::html::raster_observer, text_shaping};
#[path = "svg_rasterize_html_font_selected_face_fixture.rs"]
mod fixture;
use fixture::Fixture;
use std::cell::RefCell;
#[path = "svg_rasterize_html_font_selected_face_mutation_tests.rs"]
mod mutation;
#[path = "svg_rasterize_html_font_selected_face_scenario_tests.rs"]
mod scenarios;
use mutation::{
    MutationAttempt, check_selected_mutation_attempt, check_unselected_mutation_attempt,
};
use scenarios::{
    check_ab_alias_scope, check_cow_scope, check_fallback_scope, check_nested_scope,
    check_prefix_mutation_scope, check_unwind_scope,
};

type Backings = RefCell<Vec<fixture::WeakBytes>>;
type MappedBacking = RefCell<Option<fixture::WeakBytes>>;
type UnwindResult = std::thread::Result<Result<(), String>>;

#[test]
fn selected_face_real_ab_a_reuses_alias_after_weak_disassociation() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let backings = std::cell::RefCell::new(Vec::new());
    SvgRasterizeOps::with_html_font_render_scope(|| check_ab_alias_scope(&fixture, &backings))?;
    assert_state_empty(&fixture.context)?;
    assert_backings_released(&backings);
    Ok(())
}

#[test]
fn selected_face_real_ttc_nonzero_index_keeps_all_prefix_and_generic_contracts()
-> Result<(), String> {
    let fixture = Fixture::new()?;
    let backings = std::cell::RefCell::new(Vec::new());
    SvgRasterizeOps::with_html_font_render_scope(|| -> Result<(), String> {
        let observer = raster_observer::Scope::enter();
        let attempt = fixture.context.begin().ok_or("attempt missing")?;
        let mut database = fixture.database();
        let alias = fixture.select(&mut database, fixture.ttc)?;
        fixture.assert_face(&database, fixture.ttc, alias)?;
        assert!(database.face(alias).ok_or("TTC alias missing")?.index > 0);
        assert_eq!(fixture.select(&mut database, fixture.ttc)?, alias);
        assert!(fixture.context.finish(&database));
        assert_eq!(alias_count(&observer.snapshot(), "appended"), 1);
        assert_eq!(alias_count(&observer.snapshot(), "reused"), 1);
        *backings.borrow_mut() = mapped_backings(&fixture.context)?;
        drop(attempt);
        Ok(())
    })?;
    assert_state_empty(&fixture.context)?;
    assert_backings_released(&backings);
    Ok(())
}

#[test]
fn selected_face_real_distinct_databases_and_cow_never_confuse_same_local_id() -> Result<(), String>
{
    let fixture = Fixture::new()?;
    let backings = std::cell::RefCell::new(Vec::new());
    SvgRasterizeOps::with_html_font_render_scope(|| check_cow_scope(&fixture, &backings))?;
    assert_state_empty(&fixture.context)?;
    assert_backings_released(&backings);
    Ok(())
}

#[test]
fn selected_face_real_replace_and_absent_appearance_reject_previous_alias() -> Result<(), String> {
    for replace in [true, false] {
        let fixture = Fixture::new()?;
        let state = MutationAttempt::new();
        SvgRasterizeOps::with_html_font_render_scope(|| {
            check_selected_mutation_attempt(&fixture, &state, replace)
        })?;

        state.assert_finished(&fixture, if replace { 2 } else { 1 })?;
    }
    Ok(())
}

#[test]
fn selected_face_unselected_file_replace_rejects_at_transaction_exit_and_releases()
-> Result<(), String> {
    let fixture = Fixture::new()?;
    let state = MutationAttempt::new();
    SvgRasterizeOps::with_html_font_render_scope(|| {
        check_unselected_mutation_attempt(&fixture, &state)
    })?;

    state.assert_finished(&fixture, 2)?;
    Ok(())
}

#[test]
fn selected_face_prefix_and_generic_mutation_fail_closed() -> Result<(), String> {
    for generic in [true, false] {
        let fixture = Fixture::new()?;
        let backings = std::cell::RefCell::new(Vec::new());
        SvgRasterizeOps::with_html_font_render_scope(|| {
            check_prefix_mutation_scope(&fixture, &backings, generic)
        })?;
        assert_state_empty(&fixture.context)?;
        assert_backings_released(&backings);
    }
    Ok(())
}

#[test]
fn selected_face_unwind_clears_attempt_backing_and_restarts_without_poison() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let backings = std::cell::RefCell::new(Vec::new());
    SvgRasterizeOps::with_html_font_render_scope(|| check_unwind_scope(&fixture, &backings))?;
    assert_state_empty(&fixture.context)?;
    assert_backings_released(&backings);
    Ok(())
}

#[test]
fn selected_face_default_standalone_parse_uses_selected_route_and_final_tree_drop()
-> Result<(), String> {
    assert!(!text_shaping::known_memo_usable());
    let observer = raster_observer::Scope::enter();
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="40"><text x="4" y="24" font-family="Arial">AB</text></svg>"#;
    let tree = super::super::parse(svg, svg).map_err(|e| e.to_string())?;
    let snapshot = observer.snapshot();
    assert_default_route(&snapshot);
    let weak_db = Arc::downgrade(tree.fontdb());
    let maps = tree_backings(&tree);
    assert!(!maps.is_empty());
    let clone = tree.clone();
    drop(tree);
    assert!(weak_db.upgrade().is_some());
    assert!(maps.iter().all(|w| w.upgrade().is_some()));
    assert_standalone_pixels(&clone, svg)?;
    drop(clone);
    assert!(weak_db.upgrade().is_none());
    assert!(maps.iter().all(|w| w.upgrade().is_none()));
    Ok(())
}

#[test]
fn selected_face_real_nested_usvg_database_keeps_namespaces_distinct() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let backings = std::cell::RefCell::new(Vec::new());
    SvgRasterizeOps::with_html_font_render_scope(|| check_nested_scope(&fixture, &backings))?;
    assert_state_empty(&fixture.context)?;
    assert_backings_released(&backings);
    Ok(())
}

#[test]
fn selected_face_installed_fallback_invalidates_real_usvg_parse() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let backings = std::cell::RefCell::new(Vec::new());
    SvgRasterizeOps::with_html_font_render_scope(|| check_fallback_scope(&fixture, &backings))?;
    assert_state_empty(&fixture.context)?;
    assert_backings_released(&backings);
    Ok(())
}

fn mapped_backings(context: &Context) -> Result<Vec<fixture::WeakBytes>, String> {
    let state = context.state.lock().map_err(|_| "state poisoned")?;
    let state = state.as_ref().ok_or("attempt missing")?;
    let maps = state
        .aliases
        .iter()
        .filter_map(|alias| match &alias.face.source {
            Source::SharedFile(_, data) => Some(Arc::downgrade(data)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!maps.is_empty());
    Ok(maps)
}

fn alias_count(snapshot: &raster_observer::Snapshot, event: &str) -> usize {
    snapshot
        .events
        .iter()
        .filter(|e| e["stage"] == "selected_alias" && e["event"] == event)
        .count()
}

fn assert_state_empty(context: &Context) -> Result<(), String> {
    assert!(
        context
            .state
            .lock()
            .map_err(|_| "state poisoned")?
            .is_none()
    );
    Ok(())
}

fn assert_backings_released(backings: &Backings) {
    assert!(!backings.borrow().is_empty());
    assert!(
        backings
            .borrow()
            .iter()
            .all(|weak| weak.upgrade().is_none())
    );
}

fn assert_map_released(weak: &MappedBacking, missing: &str) -> Result<(), String> {
    assert!(weak.borrow().as_ref().ok_or(missing)?.upgrade().is_none());
    Ok(())
}

fn select_second_database(fixture: &Fixture, first_a: ID) -> Result<Arc<Database>, String> {
    let mut second = fixture.database();
    let second_b = fixture.select(&mut second, fixture.b)?;
    assert_eq!(first_a, second_b);
    fixture.assert_face(&second, fixture.b, second_b)?;
    let second_a = fixture.select(&mut second, fixture.a)?;
    assert_ne!(second_a, second_b);
    fixture.assert_face(&second, fixture.a, second_a)?;
    Ok(second)
}

fn installed_options(fixture: &Fixture, valid: &Arc<AtomicBool>) -> usvg::Options<'static> {
    let selected = Arc::new(AtomicBool::new(false));
    let mut options =
        crate::markdown::svg_rasterize::font::rasterizer_options_with_font_db(fixture.database());
    text_shaping::install_html_font_resolver(&mut options);
    fixture
        .context
        .install(&mut options, Arc::clone(valid), selected);
    options
}

fn tree_backings(tree: &usvg::Tree) -> Vec<fixture::WeakBytes> {
    tree.fontdb()
        .faces()
        .filter_map(|f| match &f.source {
            Source::SharedFile(_, data) => Some(Arc::downgrade(data)),
            _ => None,
        })
        .collect()
}

fn assert_default_route(snapshot: &raster_observer::Snapshot) {
    assert_eq!(snapshot.dropped, 0);
    assert!(alias_count(snapshot, "appended") >= 1);
    assert!(snapshot.events.iter().any(|e| e["route"] == "known_return"));
    assert!(snapshot.events.iter().any(|e| e["stage"] == "selector"
        && e["admitted"] == true
        && e["actualFace"]["sourceKind"] == "SharedFile"));
}

fn assert_standalone_pixels(clone: &usvg::Tree, svg: &str) -> Result<(), String> {
    let compatible = SvgRasterizeOps::preprocess_for_rasterizer(svg);
    let actual =
        SvgRasterizeOps::render_tree(clone, 1.0, &compatible).map_err(|e| e.to_string())?;
    let options = crate::markdown::svg_rasterize::font::html::html_rasterizer_options(svg);
    let expected =
        SvgRasterizeOps::rasterize_with_options(svg, 1.0, &options).map_err(|e| e.to_string())?;
    assert_eq!(
        (actual.width, actual.height, &actual.rgba),
        (expected.width, expected.height, &expected.rgba)
    );
    Ok(())
}

fn unwind_selected(fixture: &Fixture, weak: &MappedBacking) -> UnwindResult {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
        let _attempt = fixture.context.begin().ok_or("attempt missing")?;
        assert!(fixture.context.begin().is_none());
        let mut database = fixture.database();
        let alias = fixture.select(&mut database, fixture.a)?;
        *weak.borrow_mut() = Some(fixture.assert_face(&database, fixture.a, alias)?);
        std::panic::resume_unwind(Box::new("selected fixture unwind"));
    }))
}

#[path = "svg_rasterize_html_font_selected_face_boundary_tests.rs"]
mod boundaries;

#[path = "svg_rasterize_html_font_admitted_attempt_tests.rs"]
mod admitted_attempt_tests;
