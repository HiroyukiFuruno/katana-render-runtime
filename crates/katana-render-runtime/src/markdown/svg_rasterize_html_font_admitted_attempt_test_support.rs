use super::*;
use crate::markdown::svg_rasterize::RasterizedSvg;
use crate::markdown::svg_rasterize::font::html::admission;

pub(super) fn receipt(fixture: &Fixture, id: ID) -> Result<admission::Admission<'_>, String> {
    let face = fixture
        .context
        .original
        .face(id)
        .ok_or("original face missing")?;
    admission::admit_face(
        &fixture.context.original,
        &fixture.context.proof,
        &face.families.first().ok_or("family missing")?.0,
        "AB",
        face.weight.0,
        face.style == usvg::fontdb::Style::Italic,
    )
    .ok_or_else(|| "admission rejected".into())
}

pub(super) fn reuses_alias_but_rejects_late_font(fixture: &Fixture) -> Result<(), String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let attempt = fixture.context.begin().ok_or("attempt missing")?;
        let mut database = fixture.database();
        let alias = fixture.select(&mut database, fixture.a)?;
        let weak = fixture.assert_face(&database, fixture.a, alias)?;
        let state = fixture.context.state.lock().map_err(|_| "state poisoned")?;
        let admission = receipt(fixture, fixture.a)?;
        write_missing_font(fixture)?;
        assert_eq!(
            fixture.context.render_face(
                &mut database,
                fixture.a,
                Some(AdmittedAttempt {
                    context: &fixture.context,
                    state,
                    admission
                }),
            ),
            Some(alias)
        );
        assert!(!fixture.context.finish(&database));
        drop(attempt);
        drop(database);
        assert!(weak.upgrade().is_none());
        assert!(fixture.context.begin().is_none());
        Ok(())
    })
}

pub(super) fn selected_replacement_rejects_uncached_generation(
    fixture: &Fixture,
) -> Result<(), String> {
    let completed = std::cell::Cell::new(false);
    SvgRasterizeOps::with_html_font_render_scope(|| {
        if completed.get() {
            return Ok(());
        }
        run_selected_replacement(fixture)?;
        completed.set(true);
        Ok(())
    })
}

fn run_selected_replacement(fixture: &Fixture) -> Result<(), String> {
    let attempt = fixture.context.begin().ok_or("attempt missing")?;
    let mut database = fixture.database();
    let alias = fixture.select(&mut database, fixture.a)?;
    let weak = fixture.assert_face(&database, fixture.a, alias)?;
    let state = fixture.context.state.lock().map_err(|_| "state poisoned")?;
    let admission = receipt(fixture, fixture.a)?;
    replace_font_file(fixture)?;
    let admitted = AdmittedAttempt {
        context: &fixture.context,
        state,
        admission,
    };
    assert!(
        fixture
            .context
            .render_face(&mut database, fixture.a, Some(admitted))
            .is_none()
    );
    assert!(!fixture.context.finish(&database));
    drop(attempt);
    drop(database);
    assert!(weak.upgrade().is_none());
    Ok(())
}

fn write_missing_font(fixture: &Fixture) -> Result<(), String> {
    std::fs::write(
        fixture.root.join("absent.ttf"),
        crate::markdown::svg_rasterize::font::BUNDLED_SANS_SERIF_FONT,
    )
    .map_err(|error| error.to_string())
}

fn replace_font_file(fixture: &Fixture) -> Result<(), String> {
    let replacement = fixture.root.join("replacement.ttf");
    std::fs::copy(fixture.root.join("arial.ttf"), &replacement)
        .map_err(|error| error.to_string())?;
    std::fs::rename(replacement, fixture.root.join("noto.ttf")).map_err(|error| error.to_string())
}

fn has_event(
    snapshot: &raster_observer::Snapshot,
    matches: impl Fn(&serde_json::Value) -> bool,
) -> bool {
    snapshot.events.iter().any(matches)
}

fn assert_route_contract(snapshot: &raster_observer::Snapshot) {
    assert_eq!(snapshot.dropped, 0);
    assert!(has_event(snapshot, |event| {
        event["stage"] == "known_parse_result"
            && event["success"] == true
            && event["valid"] == false
    }));
    assert!(!has_event(snapshot, |event| event["route"] == "known_return"));
    assert!(has_event(snapshot, |event| event["route"] == "eager_reparse"));
    assert!(has_event(snapshot, |event| {
        event["stage"] == "selector" && event["route"] == "eager"
    }));
}

fn assert_same_pixels(svg: &str, actual: &RasterizedSvg) -> Result<(), String> {
    let options = crate::markdown::svg_rasterize::font::html::html_rasterizer_options(svg);
    let expected = SvgRasterizeOps::rasterize_with_options(svg, 1.0, &options)
        .map_err(|error| error.to_string())?;
    assert_eq!(
        (actual.width, actual.height, &actual.rgba),
        (expected.width, expected.height, &expected.rgba)
    );
    Ok(())
}

pub(super) fn assert_publication_barrier(
    fixture: &Fixture,
    injection: &raster_observer::LocalMissingFontScope,
    mapped: bool,
) -> Result<(), String> {
    let observer = raster_observer::Scope::enter();
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="30"><text y="20" font-family="Arial">AB</text></svg>"#;
    let actual =
        SvgRasterizeOps::rasterize_html_svg(svg, 1.0).map_err(|error| error.to_string())?;
    injection.mutation_result()?;
    assert!(injection.mutated());
    if mapped {
        assert!(injection.mappings_released());
    }
    assert_route_contract(&observer.snapshot());
    assert_same_pixels(svg, &actual)?;
    assert_state_empty(&fixture.context)
}

pub(super) fn assert_fixture_escape(
    fixture: &Fixture,
    escaped: &std::path::Path,
    selected_escapes: bool,
) -> Result<(), String> {
    let local = fixture.root.join("noto.ttf");
    let (selected, replacement, label) = if selected_escapes {
        (escaped.to_path_buf(), local, "selected font")
    } else {
        (local, escaped.to_path_buf(), "replacement font")
    };
    let error = raster_observer::LocalMissingFontScope::replace_after_admission(
        Arc::clone(&fixture.context.original),
        Arc::clone(&fixture.context.proof),
        selected,
        replacement,
    )
    .err()
    .ok_or("fixture-escaping replacement path was accepted")?;
    assert_eq!(error, format!("local {label} escaped fixture root"));
    Ok(())
}
