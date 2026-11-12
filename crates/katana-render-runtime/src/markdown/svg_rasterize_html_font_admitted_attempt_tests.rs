use super::*;
#[path = "svg_rasterize_html_font_admitted_attempt_test_support.rs"]
mod test_support;
use test_support::{assert_fixture_escape, assert_publication_barrier, receipt};

#[test]
fn selected_font_copy_failure_preserves_the_original_file() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let original = std::fs::read(fixture.root.join("noto.ttf"))
        .map_err(|error| error.to_string())?;
    std::fs::remove_file(fixture.root.join("arial.ttf")).map_err(|error| error.to_string())?;

    let error = test_support::replace_font_file(&fixture)
        .err()
        .ok_or("missing Arial source unexpectedly copied")?;
    assert!(!error.is_empty());
    assert_eq!(
        std::fs::read(fixture.root.join("noto.ttf")).map_err(|error| error.to_string())?,
        original
    );
    assert!(!fixture.root.join("replacement.ttf").exists());
    Ok(())
}

#[test]
fn selected_font_rename_failure_preserves_copied_replacement() -> Result<(), String> {
    let fixture = Fixture::new()?;
    std::fs::remove_file(fixture.root.join("noto.ttf")).map_err(|error| error.to_string())?;
    std::fs::create_dir(fixture.root.join("noto.ttf")).map_err(|error| error.to_string())?;

    let error = test_support::replace_font_file(&fixture)
        .err()
        .ok_or("directory destination unexpectedly accepted replacement")?;
    assert!(!error.is_empty());
    assert!(fixture.root.join("replacement.ttf").is_file());
    assert!(fixture.root.join("noto.ttf").is_dir());
    Ok(())
}

#[test]
fn admitted_local_missing_scope_rejects_existing_path_without_mutation() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let missing = fixture.root.join("absent.ttf");
    let sentinel = b"existing local font must remain untouched".to_vec();
    std::fs::write(&missing, &sentinel).map_err(|error| error.to_string())?;

    let error = raster_observer::LocalMissingFontScope::enter(
        Arc::clone(&fixture.context.original),
        Arc::clone(&fixture.context.proof),
        missing.clone(),
    )
    .err()
    .ok_or("existing local missing-font path was accepted")?;

    assert_eq!(
        error,
        format!(
            "local missing-font path already exists: {}",
            missing.display()
        )
    );
    assert_eq!(
        std::fs::read(&missing).map_err(|error| error.to_string())?,
        sentinel
    );
    Ok(())
}

#[test]
fn admitted_local_missing_scope_preserves_file_created_after_admission() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let missing = fixture.root.join("absent.ttf");
    let injection = raster_observer::LocalMissingFontScope::enter(
        Arc::clone(&fixture.context.original),
        Arc::clone(&fixture.context.proof),
        missing.clone(),
    )?;
    let sentinel = b"racing file must remain untouched";
    std::fs::write(&missing, sentinel).map_err(|error| error.to_string())?;
    raster_observer::local_missing_after_selector(&Database::new(), true);
    let error = injection
        .mutation_result()
        .err()
        .ok_or("late-created font was overwritten")?;
    assert!(error.starts_with("create local missing font: "));
    assert!(!injection.mutated());
    assert_eq!(
        std::fs::read(&missing).map_err(|error| error.to_string())?,
        sentinel
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn admitted_replacement_rejects_fixture_escape_symlinks() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let external = Fixture::new()?;
    let escaped = fixture.root.join("escaped.ttf");
    std::os::unix::fs::symlink(external.root.join("arial.ttf"), &escaped)
        .map_err(|error| error.to_string())?;
    assert_fixture_escape(&fixture, &escaped, true)?;
    assert_fixture_escape(&fixture, &escaped, false)
}

#[test]
fn admitted_selector_late_font_reparses_before_public_raster_and_releases() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let injection = raster_observer::LocalMissingFontScope::enter(
        Arc::clone(&fixture.context.original),
        Arc::clone(&fixture.context.proof),
        fixture.root.join("absent.ttf"),
    )?;
    std::thread::spawn(|| raster_observer::local_missing_after_selector(&Database::new(), true))
        .join()
        .map_err(|_| "isolation worker panicked")?;
    assert!(!injection.mutated());
    assert_publication_barrier(&fixture, &injection, true)
}

#[test]
fn admitted_selector_late_font_before_acquisition_cannot_publish_known_raster() -> Result<(), String>
{
    let fixture = Fixture::new()?;
    let injection = raster_observer::LocalMissingFontScope::before_acquisition(
        Arc::clone(&fixture.context.original),
        Arc::clone(&fixture.context.proof),
        fixture.root.join("absent.ttf"),
    )?;
    assert_publication_barrier(&fixture, &injection, true)
}

#[test]
fn admitted_selector_selected_replacement_before_acquisition_cannot_publish_known_raster()
-> Result<(), String> {
    let fixture = Fixture::new()?;
    let injection = raster_observer::LocalMissingFontScope::replace_after_admission(
        Arc::clone(&fixture.context.original),
        Arc::clone(&fixture.context.proof),
        fixture.root.join("arial.ttf"),
        fixture.root.join("noto.ttf"),
    )?;
    assert_publication_barrier(&fixture, &injection, false)
}

#[test]
fn admitted_receipt_reuses_provisional_alias_but_finish_rejects_late_font() -> Result<(), String> {
    test_support::reuses_alias_but_rejects_late_font(&Fixture::new()?)
}

#[test]
fn admitted_receipt_selected_replacement_still_rejects_uncached_generation() -> Result<(), String> {
    test_support::selected_replacement_rejects_uncached_generation(&Fixture::new()?)
}

#[test]
fn admitted_receipt_is_bound_to_context_database_proof_and_selected_id() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let other = Fixture::new()?;
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let attempt = fixture.context.begin().ok_or("attempt missing")?;
        let admission = receipt(&fixture, fixture.a)?;
        assert!(!admission.matches(&other.context.proof, &fixture.context.original, fixture.a));
        assert!(!admission.matches(&fixture.context.proof, &fixture.database(), fixture.a));
        assert!(!admission.matches(&fixture.context.proof, &fixture.context.original, fixture.b));
        let state = fixture.context.state.lock().map_err(|_| "state poisoned")?;
        let admitted = AdmittedAttempt {
            context: &fixture.context,
            state,
            admission,
        };
        assert!(
            other
                .context
                .render_face(&mut other.database(), fixture.a, Some(admitted))
                .is_none()
        );
        assert!(fixture.context.finish(&fixture.database()));
        drop(attempt);
        assert_state_empty(&fixture.context)?;
        Ok(())
    })
}

#[test]
fn admitted_local_missing_fixture_restores_thread_state_after_unwind() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let outcome =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
            let _injection = raster_observer::LocalMissingFontScope::enter(
                Arc::clone(&fixture.context.original),
                Arc::clone(&fixture.context.proof),
                fixture.root.join("absent.ttf"),
            )?;
            std::panic::resume_unwind(Box::new("fixture unwind"));
        }));
    assert!(outcome.is_err());
    raster_observer::local_missing_after_selector(&Database::new(), true);
    assert!(!fixture.root.join("absent.ttf").exists());
    assert_state_empty(&fixture.context)?;
    Ok(())
}

#[test]
fn admitted_installed_selector_without_active_attempt_cannot_map() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let valid = Arc::new(AtomicBool::new(true));
    let options = installed_options(&fixture, &valid);
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="30"><text y="20" font-family="Arial">AB</text></svg>"#;
    let tree = usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    assert!(!valid.load(Ordering::Relaxed));
    assert!(tree_backings(&tree).is_empty());
    assert_state_empty(&fixture.context)?;
    Ok(())
}
