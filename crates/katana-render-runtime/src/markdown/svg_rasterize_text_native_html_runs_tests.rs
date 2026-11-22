use super::test_support::{
    FONT_SIZE, INTERMEDIATE_WEIGHT, regular_shape, roboto_flex, variable_face,
};
use super::{Arc, Database, ID, Memo, Run, RunFace, RunPlan, SelectionKey, SelectionQuery};
use crate::markdown::svg_rasterize::SvgRasterizeOps;

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
fn native_run_advance_uses_real_variable_face_variations() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
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
