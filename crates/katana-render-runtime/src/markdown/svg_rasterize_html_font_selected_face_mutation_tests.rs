use super::*;
use std::cell::Cell;

fn change_selected_source(fixture: &Fixture, replace: bool) -> Result<(), String> {
    if replace {
        let replacement = fixture.root.join("replacement.ttf");
        std::fs::copy("/System/Library/Fonts/Supplemental/Arial.ttf", &replacement)
            .map_err(|e| e.to_string())?;
        std::fs::rename(replacement, fixture.root.join("noto.ttf")).map_err(|e| e.to_string())?;
    } else {
        std::fs::write(
            fixture.root.join("absent.ttf"),
            crate::markdown::svg_rasterize::font::BUNDLED_SANS_SERIF_FONT,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn assert_replaced_file_pixels(fixture: &Fixture) -> Result<(), String> {
    let path = fixture.root.join("arial.ttf");
    let file_database = replaced_file_database(&path);
    let mut binary_database = Database::new();
    binary_database.load_font_data(std::fs::read(path).map_err(|e| e.to_string())?);
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="30"><text y="20" font-family="Noto Sans">AB</text></svg>"#;
    let file_options = crate::markdown::svg_rasterize::font::rasterizer_options_with_font_db(
        Arc::new(file_database),
    );
    let binary_options = crate::markdown::svg_rasterize::font::rasterizer_options_with_font_db(
        Arc::new(binary_database),
    );
    let file_result = SvgRasterizeOps::rasterize_with_options(svg, 1.0, &file_options)
        .map_err(|e| e.to_string())?;
    let binary_result = SvgRasterizeOps::rasterize_with_options(svg, 1.0, &binary_options)
        .map_err(|e| e.to_string())?;
    assert_eq!(
        (file_result.width, file_result.height, &file_result.rgba),
        (
            binary_result.width,
            binary_result.height,
            &binary_result.rgba
        )
    );
    Ok(())
}

fn replaced_file_database(path: &std::path::Path) -> Database {
    let mut file_database = Database::new();
    let loaded = file_database.load_font_source(Source::File(path.to_path_buf()));
    assert!(!loaded.is_empty());
    assert!(
        file_database
            .faces()
            .all(|face| face.families.iter().any(|(name, _)| name == "Noto Sans"))
    );
    file_database
}

pub(super) struct MutationAttempt {
    attempts: Cell<usize>,
    first_completed: Cell<bool>,
    weak: MappedBacking,
}

impl MutationAttempt {
    pub(super) fn new() -> Self {
        Self {
            attempts: Cell::new(0),
            first_completed: Cell::new(false),
            weak: RefCell::new(None),
        }
    }

    fn retry(&self) -> bool {
        self.attempts.set(self.attempts.get() + 1);
        if self.attempts.get() > 1 {
            assert_eq!(self.attempts.get(), 2);
            assert!(!text_shaping::known_memo_usable());
            assert!(self.first_completed.get());
            return true;
        }
        assert!(text_shaping::known_memo_usable());
        false
    }

    pub(super) fn assert_finished(
        &self,
        fixture: &Fixture,
        expected_attempts: usize,
    ) -> Result<(), String> {
        assert!(self.first_completed.get());
        assert_eq!(self.attempts.get(), expected_attempts);
        assert_state_empty(&fixture.context)?;
        assert_map_released(&self.weak, "positive map missing")
    }
}

pub(super) fn check_selected_mutation_attempt(
    fixture: &Fixture,
    state: &MutationAttempt,
    replace: bool,
) -> Result<(), String> {
    if state.retry() {
        return Ok(());
    }
    let attempt = fixture.context.begin().ok_or("attempt missing")?;
    let (mut database, _) = selected_database_and_backing(fixture, state)?;
    change_selected_source(fixture, replace)?;
    assert!(
        fixture
            .context
            .render_face(&mut database, fixture.a, None)
            .is_none()
    );
    assert!(!fixture.context.finish(&database));
    drop(attempt);
    drop(database);
    assert_state_empty(&fixture.context)?;
    assert_map_released(&state.weak, "positive map missing")?;
    state.first_completed.set(true);
    Ok(())
}

fn selected_database_and_backing(
    fixture: &Fixture,
    state: &MutationAttempt,
) -> Result<(Arc<Database>, ID), String> {
    let mut database = fixture.database();
    let alias = fixture.select(&mut database, fixture.a)?;
    *state.weak.borrow_mut() = Some(fixture.assert_face(&database, fixture.a, alias)?);
    Ok((database, alias))
}

pub(super) fn check_unselected_mutation_attempt(
    fixture: &Fixture,
    state: &MutationAttempt,
) -> Result<(), String> {
    if state.retry() {
        return Ok(());
    }
    let attempt = fixture.context.begin().ok_or("attempt missing")?;
    let (mut database, alias) = selected_database_and_backing(fixture, state)?;
    replace_unselected_source(fixture)?;
    assert_eq!(
        fixture.context.render_face(&mut database, fixture.a, None),
        Some(alias)
    );
    assert!(!fixture.context.finish(&database));
    drop(attempt);
    drop(database);
    assert_map_released(&state.weak, "positive map missing")?;
    assert!(fixture.context.begin().is_none());
    assert_replaced_file_pixels(fixture)?;
    state.first_completed.set(true);
    Ok(())
}

fn replace_unselected_source(fixture: &Fixture) -> Result<(), String> {
    let replacement = fixture.root.join("replacement.ttf");
    std::fs::write(
        &replacement,
        crate::markdown::svg_rasterize::font::BUNDLED_SANS_SERIF_FONT,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(replacement, fixture.root.join("arial.ttf")).map_err(|e| e.to_string())?;
    Ok(())
}
