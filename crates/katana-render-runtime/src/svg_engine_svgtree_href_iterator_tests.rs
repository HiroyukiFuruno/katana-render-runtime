use super::{node_by_id, parse_document};

const LONG_CYCLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
<g id="a" xlink:href="#b"/><g id="b" xlink:href="#c"/>
<g id="c" xlink:href="#d"/><g id="d" xlink:href="#e"/>
<g id="e" xlink:href="#f"/><g id="f" xlink:href="#g"/>
<g id="g" xlink:href="#h"/><g id="h" xlink:href="#e"/>
</svg>"##;
const INLINE_HISTORY_OVERFLOW_SEQUENCE: [&str; 8] = ["a", "b", "c", "d", "e", "f", "g", "h"];
const CYCLE_REPETITION_PROBE: usize = 1;
const CLONE_PREFIX_LENGTH: usize = 6;

#[test]
fn iterator_clone_and_debug_preserve_traversal_state() -> Result<(), String> {
    let document = parse_document(LONG_CYCLE_SVG)?;
    let mut original = node_by_id(&document, "a")?.href_iter();
    for id in &INLINE_HISTORY_OVERFLOW_SEQUENCE[..CLONE_PREFIX_LENGTH] {
        assert_eq!(original.next().map(|node| node.element_id()), Some(*id));
    }
    let mut clone = original.clone();
    assert!(format!("{original:?}").contains("HrefIter"));
    for id in &INLINE_HISTORY_OVERFLOW_SEQUENCE[CLONE_PREFIX_LENGTH..] {
        assert_eq!(original.next().map(|node| node.element_id()), Some(*id));
        assert_eq!(clone.next().map(|node| node.element_id()), Some(*id));
    }
    assert!(original.next().is_none());
    assert!(clone.next().is_none());
    Ok(())
}

#[test]
fn iterator_stops_a_cycle_after_inline_history_spills() -> Result<(), String> {
    let document = parse_document(LONG_CYCLE_SVG)?;
    let mut iterator = node_by_id(&document, "a")?.href_iter();
    let actual = iterator
        .by_ref()
        .take(INLINE_HISTORY_OVERFLOW_SEQUENCE.len() + CYCLE_REPETITION_PROBE)
        .map(|node| node.element_id())
        .collect::<Vec<_>>();
    assert_eq!(actual, INLINE_HISTORY_OVERFLOW_SEQUENCE);
    assert!(iterator.next().is_none());
    Ok(())
}
