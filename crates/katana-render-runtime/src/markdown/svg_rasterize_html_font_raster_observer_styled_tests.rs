use super::{Scope, install_eager, real_database};
use resvg::usvg::{self, fontdb::Database};
use rstest::rstest;
use std::sync::Arc;

const BASE_FONT: &[u8] = include_bytes!(
    "../../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/Yellowtail-Regular.ttf"
);

fn requested_font(database: &Arc<Database>) -> Result<usvg::Font, String> {
    let options = usvg::Options {
        fontdb: Arc::clone(database),
        ..usvg::Options::default()
    };
    let input = "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><text id='run' font-family='Noto Sans'>A</text></svg>";
    let tree = usvg::Tree::from_str(input, &options).map_err(|error| error.to_string())?;
    let Some(usvg::Node::Text(text)) = tree.node_by_id("run") else {
        return Err("fixture text missing".to_owned());
    };
    text.chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .map(|span| span.font().clone())
        .ok_or("fixture font missing".to_owned())
}

fn styled_options() -> usvg::Options<'static> {
    let mut options = usvg::Options::default();
    let delegate = usvg::FontResolver::default_fallback_selector();
    options.font_resolver.select_styled_fallback =
        Some(Box::new(move |_, character, excluded, database| {
            delegate(character, excluded, database)
        }));
    install_eager(&mut options);
    options
}

fn fixture_database() -> Result<(Arc<Database>, usvg::fontdb::ID, usvg::fontdb::ID), String> {
    let (mut database, candidate, _bytes) = real_database()?;
    let ids = Arc::make_mut(&mut database)
        .load_font_source(usvg::fontdb::Source::Binary(Arc::new(BASE_FONT.to_vec())));
    let [base] = ids.as_slice() else {
        return Err("base face missing".to_owned());
    };
    Ok((database, *base, candidate))
}

#[rstest]
#[case::selected(false)]
#[case::excluded(true)]
fn styled_fallback_preserves_real_selector_result_and_event(
    #[case] excluded: bool,
) -> Result<(), String> {
    let (mut database, base, candidate) = fixture_database()?;
    let font = requested_font(&database)?;
    let mut excluded_ids = vec![base];
    if excluded {
        excluded_ids.push(candidate);
    }
    let direct = usvg::FontResolver::default_fallback_selector()('A', &excluded_ids, &mut database);
    assert_eq!(direct, if excluded { None } else { Some(candidate) });
    let options = styled_options();
    let observer = Scope::enter();
    let styled = options
        .font_resolver
        .select_styled_fallback
        .as_ref()
        .ok_or("styled resolver missing")?;
    let actual = styled(&font, 'A', &excluded_ids, &mut database);
    assert_eq!(actual, direct);
    assert_styled_event(&observer.snapshot(), &font, &excluded_ids, actual)?;
    Ok(())
}

fn assert_styled_event(
    snapshot: &super::super::Snapshot,
    font: &usvg::Font,
    excluded: &[usvg::fontdb::ID],
    actual: Option<usvg::fontdb::ID>,
) -> Result<(), String> {
    let [event] = snapshot.events.as_slice() else {
        return Err("expected one event".to_owned());
    };
    assert_eq!(snapshot.dropped, 0);
    assert_eq!(
        snapshot.operation_counters()["operations"]["counts"]["eager_fallback"],
        1
    );
    assert_eq!(event["stage"], "fallback");
    assert_eq!(event["route"], "eager");
    assert_eq!(event["character"], "A");
    assert_eq!(event["codepoint"], u32::from('A'));
    assert_eq!(event["requestedWeight"], font.weight());
    assert_eq!(event["requestedStyle"], format!("{:?}", font.style()));
    assert_eq!(event["requestedStretch"], format!("{:?}", font.stretch()));
    assert_face_event(event, excluded, actual);
    Ok(())
}

fn assert_face_event(
    event: &serde_json::Value,
    excluded: &[usvg::fontdb::ID],
    actual: Option<usvg::fontdb::ID>,
) {
    assert_eq!(
        event["excludedLocalIds"],
        serde_json::json!(
            excluded
                .iter()
                .map(|id| format!("{id:?}"))
                .collect::<Vec<_>>()
        )
    );
    match actual {
        Some(id) => {
            assert_eq!(
                event["actualFace"]["databaseFaceIdLocalOnly"],
                format!("{id:?}")
            );
            assert_eq!(event["actualFace"]["sourceKind"], "Binary");
        }
        None => assert!(event["actualFace"].is_null()),
    }
}
