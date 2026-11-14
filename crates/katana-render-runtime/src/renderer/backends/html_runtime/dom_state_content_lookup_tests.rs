use super::super::HtmlDomBridgeState;
use super::{lookup_attribute, lookup_markup, lookup_path};
use crate::renderer::backends::html_document::HtmlDocument;
use crate::renderer::backends::html_runtime::types::DomValue;

const ANCHOR_HASH_BASE: &str = "https://example.test/docs/index.html#current";
const ANCHOR_HASH_CASES: &[(&str, &str, &str)] = &[
    ("relative", ANCHOR_HASH_BASE, "#part%20one"),
    ("absolute", "invalid base", "#target"),
    ("no-fragment", ANCHOR_HASH_BASE, ""),
    ("empty-fragment", ANCHOR_HASH_BASE, ""),
    ("empty", ANCHOR_HASH_BASE, ""),
    ("missing", ANCHOR_HASH_BASE, ""),
    ("invalid", ANCHOR_HASH_BASE, ""),
    ("area", ANCHOR_HASH_BASE, "#map"),
];

#[test]
fn anchor_hash_resolves_url_fragments_for_html_anchors() -> Result<(), String> {
    let state = anchor_hash_state();
    assert_anchor_hash_cases(&state)?;
    assert!(matches!(
        anchor_hash(&state, "plain", ANCHOR_HASH_BASE)?,
        DomValue::Undefined
    ));
    assert!(
        matches!(state.dispatch("anchorHash", &[]), Err(error) if error == "missing DOM argument 0")
    );
    Ok(())
}

#[test]
fn anchor_hash_requires_a_base_and_handles_invalid_relative_base() -> Result<(), String> {
    let state = anchor_hash_state();
    let node = anchor_node(&state, "relative")?;

    assert!(matches!(
        state.dispatch("anchorHash", &[node.to_string()]),
        Err(error) if error == "missing DOM argument 1"
    ));
    assert!(matches!(
        anchor_hash(&state, "relative", "invalid base")?,
        DomValue::String(value) if value.is_empty()
    ));
    Ok(())
}

#[test]
fn svg_anchor_is_not_treated_as_an_html_anchor() -> Result<(), String> {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse(
        "<svg><a id=svg-anchor href='#target'></a></svg>",
    ));
    assert!(matches!(
        anchor_hash(&state, "svg-anchor", ANCHOR_HASH_BASE)?,
        DomValue::Undefined
    ));
    Ok(())
}

fn anchor_hash_state() -> HtmlDomBridgeState {
    HtmlDomBridgeState::new(HtmlDocument::parse(
        "<a id=relative href='guide#part%20one'></a><a id=absolute href='https://other.test/path#target'></a><a id=no-fragment href='guide'></a><a id=empty-fragment href='#'></a><a id=empty href=''></a><a id=missing></a><a id=invalid href='http://['></a><area id=area href='#map'><div id=plain href='#ignored'></div>",
    ))
}

fn assert_anchor_hash_cases(state: &HtmlDomBridgeState) -> Result<(), String> {
    for (id, base_href, expected) in ANCHOR_HASH_CASES {
        assert!(
            matches!(
                anchor_hash(state, id, base_href)?,
                DomValue::String(value) if value == *expected
            ),
            "anchor #{id}"
        );
    }
    Ok(())
}

fn anchor_node(state: &HtmlDomBridgeState, id: &str) -> Result<u64, String> {
    state
        .document
        .borrow_mut()
        .get_element_by_id(id)
        .ok_or("test anchor must be indexed")
        .map_err(str::to_string)
}

fn anchor_hash(state: &HtmlDomBridgeState, id: &str, base: &str) -> Result<DomValue, String> {
    let node = anchor_node(state, id)?;
    state.dispatch("anchorHash", &[node.to_string(), base.to_string()])
}

#[test]
fn content_lookup_helpers_reject_unknown_operations_at_their_boundaries() -> Result<(), String> {
    let mut document = HtmlDocument::parse("<main id=root><p>text</p></main>");
    let node = document
        .query_selector("#root")
        .ok_or("root must be indexed")?
        .to_string();
    let args = [node.clone()];
    assert!(
        matches!(lookup_markup(&document, "unknown", &args), Err(error) if error.contains("unsupported markup"))
    );
    assert!(
        matches!(lookup_attribute(&document, "unknown", &args), Err(error) if error.contains("unsupported attribute"))
    );
    assert!(
        matches!(lookup_path(&document, "unknown", &args), Err(error) if error.contains("unsupported path"))
    );
    Ok(())
}

#[test]
fn content_lookup_propagates_node_errors() {
    let document = HtmlDocument::parse("<main id=root><p>text</p></main>");
    let arguments = ["999".to_string()];

    assert!(matches!(
        lookup_markup(&document, "textContent", &arguments),
        Err(error) if error == "HTML node 999 does not exist"
    ));
    for (arguments, expected) in [
        (vec![], "missing DOM argument 0"),
        (vec!["invalid".to_string()], "invalid HTML node id: invalid"),
    ] {
        assert!(matches!(
            lookup_attribute(&document, "tagName", &arguments),
            Err(error) if error == expected
        ));
    }
}

#[test]
fn anchor_hash_propagates_an_unknown_node() {
    let state = anchor_hash_state();
    assert!(matches!(
        state.dispatch(
            "anchorHash",
            &["9999".to_string(), ANCHOR_HASH_BASE.to_string()]
        ),
        Err(error) if error == "HTML node 9999 does not exist"
    ));
}
