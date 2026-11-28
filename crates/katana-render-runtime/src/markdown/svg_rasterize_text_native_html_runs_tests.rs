use super::test_support::{
    FONT_SIZE, INTERMEDIATE_WEIGHT, duplicate_width_face, regular_shape, roboto_flex, variable_face,
};
use super::{Arc, Database, ID, Memo, Run, RunFace, RunPlan, SelectionKey, SelectionQuery};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use std::{cell::OnceCell, error::Error};

#[test]
fn unsupported_native_fallback_keeps_the_existing_cjk_run_and_memoizes_the_miss() {
    let database = Database::new();
    let old_id = ID::dummy();
    let query = SelectionQuery {
        database: &database,
        base: old_id,
        system_ui: true,
        native: OnceCell::new(),
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
fn native_fallback_reuses_one_base_query_for_different_characters() -> Result<(), Box<dyn Error>> {
    let database = Database::new();
    let query = SelectionQuery {
        database: &database,
        base: ID::dummy(),
        system_ui: true,
        native: OnceCell::new(),
        key: SelectionKey {
            base: "System Font".into(),
            size: FONT_SIZE.to_bits(),
            weight: 400,
            language: Some("ja".into()),
        },
    };
    assert!(query.native.get().is_none());
    let first = query.native_query().ok_or("native base font unavailable")?;
    assert!(first.select('骨').is_some());
    let second = query.native_query().ok_or("native base font unavailable")?;
    assert!(std::ptr::eq(first, second));
    assert!(second.select('実').is_some());
    Ok(())
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
