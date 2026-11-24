use super::test_support::{
    FONT_SIZE, INTERMEDIATE_WEIGHT, duplicate_width_face, regular_shape, roboto_flex, variable_face,
};
use super::{
    Arc, Database, ID, Memo, Run, RunFace, RunPlan, SelectedFace, SelectionKey, SelectionQuery,
    choose_selected_face,
};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use crate::markdown::svg_rasterize::font_file_fixture;
use resvg::usvg::fontdb::{FaceInfo, Source};
use std::error::Error;

#[test]
fn unsupported_native_fallback_keeps_the_existing_cjk_run_and_memoizes_the_miss() {
    let database = Database::new();
    let old_id = ID::dummy();
    let query = SelectionQuery {
        database: &database,
        base: old_id,
        system_ui: true,
        key: SelectionKey {
            base: "System Font".into(),
            size: FONT_SIZE.to_bits(),
            weight: INTERMEDIATE_WEIGHT,
            language: None,
        },
    };
    let mut memo = Memo::default();
    for _ in 0..2 {
        assert!(
            matches!(query.select(&mut memo, '装', old_id, &[]), RunFace::Existing(id) if id == old_id)
        );
    }
}

#[test]
fn same_file_selection_keeps_existing_and_shared_selection_uses_native_alias()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let fixture = font_file_fixture::create(
        directory.path().join("font.ttf"),
        font_file_fixture::FONT_BYTES,
    )?;
    let old = fixture
        .database
        .face(fixture.face_id)
        .ok_or("original face missing")?;
    let old_id = fixture.face_id;
    let file_selected = selected_face(Arc::new(fixture.database.clone()), old_id);
    assert_keeps_face(old, file_selected);
    let (shared, shared_id) = shared_database(&fixture)?;
    let new = shared.face(shared_id).ok_or("shared face info missing")?;
    let shared_selected = selected_face(Arc::clone(&shared), shared_id);
    assert_keeps_face(new, Arc::clone(&shared_selected));
    assert!(matches!(
        choose_selected_face(old_id, Some(old), shared_selected),
        RunFace::Native(selected) if Arc::ptr_eq(&selected.database, &shared)
    ));
    Ok(())
}

fn assert_keeps_face(existing: &FaceInfo, selected: Arc<SelectedFace>) {
    assert!(matches!(
        choose_selected_face(existing.id, Some(existing), selected),
        RunFace::Existing(id) if id == existing.id
    ));
}

fn selected_face(database: Arc<Database>, id: ID) -> Arc<SelectedFace> {
    Arc::new(SelectedFace {
        database,
        id,
        alias: "fixture".into(),
        variations: Vec::new(),
    })
}

fn shared_database(
    fixture: &font_file_fixture::FontFileFixture,
) -> Result<(Arc<Database>, ID), &'static str> {
    let mut database = Database::new();
    let ids = database.load_font_source(Source::SharedFile(
        fixture.path.clone(),
        Arc::new(font_file_fixture::FONT_BYTES.to_vec()),
    ));
    let id = *ids.first().ok_or("shared face missing")?;
    Ok((Arc::new(database), id))
}

#[test]
fn native_run_advance_uses_real_variable_face_variations() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let fixture = roboto_flex(&directory)?;
    let plan = RunPlan {
        database: Arc::new(Database::new()),
        runs: vec![Run {
            face: RunFace::Native(Arc::new(variable_face(&fixture))),
            text: "A".into(),
        }],
    };
    let advance =
        SvgRasterizeOps::with_html_font_render_scope(|| plan.advance(regular_shape(), None));
    assert!(advance.is_some_and(|value| value > 0.0));
    Ok(())
}

#[test]
fn native_run_advance_rejects_a_real_face_with_duplicate_variations() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let fixture = roboto_flex(&directory)?;
    let plan = RunPlan {
        database: Arc::new(Database::new()),
        runs: vec![Run {
            face: RunFace::Native(Arc::new(duplicate_width_face(&fixture))),
            text: "A".into(),
        }],
    };
    let advance =
        SvgRasterizeOps::with_html_font_render_scope(|| plan.advance(regular_shape(), None));
    assert_eq!(advance, None);
    Ok(())
}
