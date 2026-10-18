use super::*;
use std::sync::atomic::Ordering;

#[test]
fn frame_scope_rejects_changed_generation_and_actual_lazy_read_poison_across_inners()
-> TestResult<()> {
    let path = temp_font_path("frame-generation-poison");
    std::fs::write(&path, FONT_BYTES)?;
    let (database, face) = file_database(&path)?;
    let result = SvgRasterizeOps::with_html_font_face_cache_scope(|| {
        changed_generation_then_poison(&path, &database, face)
    });
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    assert_released();
    result
}

fn assert_file_retention(database: &Database, face: ID, font: &FontHandle) {
    if font_source_generation(database, face).durable_reusable() {
        assert_retained_owner(1, font);
    } else {
        assert_all_handles_cleared();
    }
}

fn changed_generation_then_poison(path: &Path, database: &Database, face: ID) -> TestResult<()> {
    let old = inner_font(database, face)?;
    assert_file_retention(database, face, &old);
    std::fs::write(path, [FONT_BYTES, &[0_u8][..]].concat())?;
    assert!(!commit_file_font(database, face, &old));
    assert_all_handles_cleared();
    let fresh = fresh_lazy_font(database, face)?;
    assert!(!std::ptr::eq(old.metrics(), fresh.metrics()));
    assert_eq!(old.metrics(), fresh.metrics());
    assert_file_retention(database, face, &fresh);
    std::fs::remove_file(path)?;
    SvgRasterizeOps::with_html_font_render_scope(|| {
        check_actual_lazy_poison(database, face, &fresh)
    })?;
    Ok(())
}

fn fresh_lazy_font(database: &Database, face: ID) -> Result<FontHandle, String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let font = font_for_face(database, face).ok_or("fresh real lazy face missing")?;
        assert!(font.metrics().h_line().is_some());
        assert!(commit_file_font(database, face, &font));
        Ok(font)
    })
}

fn check_actual_lazy_poison(
    database: &Database,
    face: ID,
    fresh: &FontHandle,
) -> Result<(), String> {
    /* WHY: metricsで未読のpost tableを実際に取得させ、flag注入ではなくIO失敗を観測する。 */
    let _ = fresh.style_metrics();
    assert!(
        fresh
            .poisoned
            .as_ref()
            .ok_or("real lazy poison flag missing")?
            .load(Ordering::Acquire)
    );
    assert!(!commit_file_font(database, face, fresh));
    assert_all_handles_cleared();
    assert!(font_for_face(database, face).is_none());
    Ok(())
}
