use super::*;

pub(super) fn check_ab_alias_scope(fixture: &Fixture, backings: &Backings) -> Result<(), String> {
    let observer = raster_observer::Scope::enter();
    let attempt = fixture.context.begin().ok_or("attempt missing")?;
    let mut database = fixture.database();
    let a = fixture.select(&mut database, fixture.a)?;
    let a_data = fixture.assert_face(&database, fixture.a, a)?;
    let b = fixture.select(&mut database, fixture.b)?;
    fixture.assert_face(&database, fixture.b, b)?;
    assert_eq!(fixture.select(&mut database, fixture.a)?, a);
    assert_eq!(database.len(), fixture.context.original.len() + 2);
    assert!(fixture.context.finish(&database));
    assert_eq!(alias_count(&observer.snapshot(), "appended"), 2);
    assert_eq!(alias_count(&observer.snapshot(), "reused"), 1);
    *backings.borrow_mut() = mapped_backings(&fixture.context)?;
    drop(attempt);
    assert_state_empty(&fixture.context)?;
    assert!(a_data.upgrade().is_some());
    drop(database);
    assert!(a_data.upgrade().is_none());
    Ok(())
}

pub(super) fn check_cow_scope(fixture: &Fixture, backings: &Backings) -> Result<(), String> {
    let attempt = fixture.context.begin().ok_or("attempt missing")?;
    let mut first = fixture.database();
    let first_a = fixture.select(&mut first, fixture.a)?;
    let retained = Arc::clone(&first);
    let first_b = fixture.select(&mut first, fixture.b)?;
    assert!(!Arc::ptr_eq(&first, &retained));
    fixture.assert_face(&first, fixture.b, first_b)?;
    assert_eq!(fixture.select(&mut first, fixture.a)?, first_a);
    let second = select_second_database(fixture, first_a)?;
    assert!(fixture.context.finish(&first));
    assert!(fixture.context.finish(&second));
    assert!(fixture.context.finish(&retained));
    *backings.borrow_mut() = mapped_backings(&fixture.context)?;
    drop(attempt);
    Ok(())
}

pub(super) fn check_unwind_scope(fixture: &Fixture, backings: &Backings) -> Result<(), String> {
    let weak = std::cell::RefCell::new(None);
    let result = unwind_selected(fixture, &weak);
    assert!(result.is_err());
    assert_map_released(&weak, "positive map branch missing")?;
    assert_state_empty(&fixture.context)?;
    let attempt = fixture.context.begin().ok_or("restart missing")?;
    let mut database = fixture.database();
    fixture.select(&mut database, fixture.b)?;
    assert!(fixture.context.finish(&database));
    *backings.borrow_mut() = mapped_backings(&fixture.context)?;
    drop(attempt);
    Ok(())
}

pub(super) fn check_nested_scope(fixture: &Fixture, backings: &Backings) -> Result<(), String> {
    let observer = raster_observer::Scope::enter();
    let valid = Arc::new(AtomicBool::new(true));
    let options = installed_options(fixture, &valid);
    let attempt = fixture.context.begin().ok_or("attempt missing")?;
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="30"><text y="20" font-family="Arial">AB</text></svg>"#;
    let outer = usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    let nested =
        usvg::Tree::from_data_nested(svg.as_bytes(), &options).map_err(|e| e.to_string())?;
    assert!(!Arc::ptr_eq(outer.fontdb(), nested.fontdb()));
    assert!(valid.load(Ordering::Relaxed));
    assert!(fixture.context.finish(outer.fontdb()));
    assert!(fixture.context.finish(nested.fontdb()));
    assert_eq!(alias_count(&observer.snapshot(), "appended"), 2);
    let maps = [&outer, &nested]
        .into_iter()
        .flat_map(tree_backings)
        .collect::<Vec<_>>();
    assert_eq!(maps.len(), 2);
    *backings.borrow_mut() = mapped_backings(&fixture.context)?;
    drop(attempt);
    drop(options);
    drop(outer);
    assert!(maps.iter().all(|w| w.upgrade().is_some()));
    drop(nested);
    assert!(maps.iter().all(|w| w.upgrade().is_none()));
    Ok(())
}

pub(super) fn check_fallback_scope(fixture: &Fixture, backings: &Backings) -> Result<(), String> {
    let observer = raster_observer::Scope::enter();
    let valid = Arc::new(AtomicBool::new(true));
    let options = installed_options(fixture, &valid);
    let attempt = fixture.context.begin().ok_or("attempt missing")?;
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="30"><text y="20" font-family="Arial">AB𠀀</text></svg>"#;
    let tree = usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    assert!(alias_count(&observer.snapshot(), "appended") >= 1);
    assert!(
        observer
            .snapshot()
            .events
            .iter()
            .any(|e| e["stage"] == "fallback")
    );
    assert!(!valid.load(Ordering::Relaxed));
    drop(tree);
    *backings.borrow_mut() = mapped_backings(&fixture.context)?;
    drop(attempt);
    Ok(())
}

pub(super) fn check_prefix_mutation_scope(
    fixture: &Fixture,
    backings: &Backings,
    generic: bool,
) -> Result<(), String> {
    let attempt = fixture.context.begin().ok_or("attempt missing")?;
    let mut database = fixture.database();
    fixture.select(&mut database, fixture.a)?;
    if generic {
        Arc::make_mut(&mut database).set_serif_family("mutated");
    } else {
        Arc::make_mut(&mut database).remove_face(fixture.b);
    }
    assert!(
        fixture
            .context
            .render_face(&mut database, fixture.a, None)
            .is_none()
    );
    assert!(!fixture.context.finish(&database));
    *backings.borrow_mut() = mapped_backings(&fixture.context)?;
    drop(attempt);
    Ok(())
}
