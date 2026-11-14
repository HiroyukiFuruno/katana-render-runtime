use super::*;

#[test]
fn frame_scope_keeps_two_real_file_slots_and_admits_third_within_budget_across_inners()
-> TestResult<()> {
    let paths = [
        temp_font_path("frame-budget-a"),
        temp_font_path("frame-budget-b"),
        temp_font_path("frame-budget-c"),
    ];
    let result =
        SvgRasterizeOps::with_html_font_face_cache_scope(|| populate_budget_inners(&paths));
    for path in paths {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    assert_released();
    result
}

fn populate_budget_inners(paths: &[PathBuf; BACKED_FONT_COUNT]) -> TestResult<()> {
    let mut held = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        let length = if index < FIRST_BACKED_FONT_COUNT {
            FIRST_BACKED_FONT_BYTES
        } else {
            NEXT_BACKED_FONT_BYTES
        };
        write_budget_file(path, length)?;
        let (database, face) = file_database(path)?;
        let durable = font_source_generation(&database, face).durable_reusable();
        let font = inner_font(&database, face)?;
        if durable {
            assert_retained_owner(1, &font);
            assert_budget_slots(index, length, &held);
        } else {
            assert_all_handles_cleared();
        }
        held.push(font);
    }
    Ok(())
}

fn write_budget_file(path: &Path, length: usize) -> TestResult<()> {
    std::fs::write(path, FONT_BYTES)?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .set_len(length as u64)?;
    Ok(())
}

fn assert_budget_slots(index: usize, length: usize, held: &[FontHandle]) {
    ACTIVE_SCOPE.with(|cache| {
        let cache = cache.borrow();
        assert_eq!(
            cache
                .entry
                .as_ref()
                .map(|entry| entry.font.retained_source_bytes),
            Some(length)
        );
        if index > 0 {
            let previous = &held[index - 1];
            assert!(
                cache
                    .spare
                    .as_ref()
                    .is_some_and(|entry| std::ptr::eq(entry.font.metrics(), previous.metrics()))
            );
        }
    });
    if index == 2 {
        assert_oldest_evicted(&held[0]);
    }
}

fn assert_oldest_evicted(evicted: &FontHandle) {
    ACTIVE_SCOPE.with(|cache| {
        let cache = cache.borrow();
        assert!(
            !cache
                .entry
                .iter()
                .chain(cache.spare.iter())
                .any(|entry| { std::ptr::eq(entry.font.metrics(), evicted.metrics()) })
        );
    });
}
