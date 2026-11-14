use super::*;

fn requested_tree(database: &Arc<Database>) -> TestResult<usvg::Tree> {
    let svg = "<svg xmlns='http://www.w3.org/2000/svg'><text id='x' font-family='Noto Sans'><tspan font-weight='600'>A</tspan><tspan font-weight='700'>A</tspan></text></svg>";
    let options = usvg::Options {
        fontdb: Arc::clone(database),
        ..usvg::Options::default()
    };
    Ok(usvg::Tree::from_str(svg, &options)?)
}

fn requested_font(tree: &usvg::Tree) -> TestResult<&usvg::Font> {
    let Some(usvg::Node::Text(text)) = tree.node_by_id("x") else {
        return Err("text missing".into());
    };
    Ok(text
        .chunks()
        .first()
        .ok_or("chunk missing")?
        .spans()
        .first()
        .ok_or("span missing")?
        .font())
}

fn styled_options(calls: Arc<AtomicUsize>) -> usvg::Options<'static> {
    let mut options = usvg::Options::default();
    options.font_resolver.select_styled_fallback = Some(Box::new(move |_, _, _, database| {
        calls.fetch_add(1, Ordering::Relaxed);
        database.faces().next().map(|face| face.id)
    }));
    install(&mut options);
    options
}

fn select_styled(
    options: &usvg::Options<'_>,
    font: &usvg::Font,
    database: &Arc<Database>,
) -> Option<ID> {
    let mut database = Arc::clone(database);
    options.font_resolver.select_styled_fallback.as_ref()?(font, 'x', &[], &mut database)
}

#[cfg(target_os = "macos")]
#[test]
fn physical_face_memo_rejects_other_source_generation() -> TestResult<()> {
    let first_file = FontFile::create()?;
    let second_file = FontFile::create()?;
    let first_db = file_database(&first_file.0);
    let second_db = file_database(&second_file.0);
    let first_id = first_face(&first_db)?;
    let second_id = first_face(&second_db)?;
    let first = super::super::super::font::font_source_generation(&first_db, first_id);
    let second = super::super::super::font::font_source_generation(&second_db, second_id);
    assert_ne!(first, second);
    super::super::super::with_validated_tree_parse(|| {
        super::super::entries::add_database(&first_db);
        super::super::entries::physical_face_store(&first_db, first_id, first.clone(), true);
        assert_eq!(
            super::super::entries::physical_face_lookup(&first_db, first_id, &first),
            Some(true)
        );
        assert_eq!(
            super::super::entries::physical_face_lookup(&first_db, first_id, &second),
            None
        );
        assert_eq!(
            super::super::entries::physical_face_lookup(&second_db, second_id, &first),
            None
        );
    });
    Ok(())
}

#[test]
fn styled_file_generation_change_retries_and_does_not_reuse_stale_choice() -> TestResult<()> {
    let file = FontFile::create()?;
    let database = file_database(&file.0);
    let tree = requested_tree(&database)?;
    let font = requested_font(&tree)?;
    let expected = first_face(&database)?;
    let durable = super::super::super::font::file_stamp_durable_reusable(&file.0);
    let calls = Arc::new(AtomicUsize::new(0));
    let options = styled_options(Arc::clone(&calls));
    let attempts = AtomicUsize::new(0);
    let mut mutation = Ok(());
    let result = super::super::super::with_validated_tree_parse(|| {
        let attempt = attempts.fetch_add(1, Ordering::Relaxed);
        let first = select_styled(&options, font, &database);
        let second = select_styled(&options, font, &database);
        if attempt == 0 {
            mutation = std::fs::write(&file.0, b"new font generation");
        }
        (first, second)
    });
    mutation?;
    assert_eq!(attempts.load(Ordering::Relaxed), 1 + usize::from(durable));
    assert_eq!(result, (Some(expected), Some(expected)));
    assert_eq!(calls.load(Ordering::Relaxed), if durable { 3 } else { 2 });
    Ok(())
}
