#[test]
fn width_only_cache_entry_gains_svg_metrics_from_real_face() -> Result<(), String> {
    let database = bundled_font_db();
    let face = matching_font_face(&database, "Noto Sans", 400, false)
        .ok_or("bundled Noto Sans must be available")?;
    let font = FontShape::new(16.0, 400, false);
    let text = "SVG cache upgrade";
    let advance = super::shape_text_with_face(&database, face, text, font, None)
        .ok_or("the bundled font must shape the run")?;
    let key = ShapedRunKey::new(face, text, font, None);
    SHAPED_RUN_CACHE.with(|cache| cache.borrow_mut().clear());
    let generation = super::font_source_generation(&database, face);
    super::cache_shaped_run_width(&database, key.clone(), advance, generation);

    let width_only = cached_shaped_run_record(&database, &key)
        .ok_or("the seeded width-only entry must be available")?;
    assert!(width_only.svg_endpoints.is_none());
    let metrics = super::cached_svg_run(&database, face, text, font)
        .ok_or("SVG metrics must be measurable for the real face")?;
    assert_eq!(metrics.advance, advance);
    assert!(metrics.ascent.is_finite());
    assert!(metrics.descent.is_finite());

    let upgraded =
        cached_shaped_run_record(&database, &key).ok_or("the SVG metrics entry must be cached")?;
    assert_eq!(
        upgraded.svg_endpoints,
        Some((metrics.ascent, metrics.descent))
    );
    Ok(())
}
