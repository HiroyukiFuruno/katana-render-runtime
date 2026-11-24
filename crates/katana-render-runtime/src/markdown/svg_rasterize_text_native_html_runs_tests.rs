use super::test_support::{
    FONT_SIZE, INTERMEDIATE_WEIGHT, duplicate_width_face, regular_shape, roboto_flex, variable_face,
};
use super::{
    Arc, Database, ID, Memo, Run, RunFace, RunPlan, SelectionKey, SelectionQuery,
    reuse_existing_source,
};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use crate::markdown::svg_rasterize::font_file_fixture;
use resvg::usvg::fontdb::Source;
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
fn shared_selected_face_uses_its_alias_even_when_the_physical_face_matches()
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
    assert!(reuse_existing_source(Some(old), Some(old), &[]));
    let (shared, shared_id) = shared_database(&fixture)?;
    let new = shared.face(shared_id).ok_or("shared face info missing")?;
    assert!(!reuse_existing_source(Some(old), Some(new), &[]));
    assert!(reuse_existing_source(Some(new), Some(new), &[]));
    Ok(())
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
