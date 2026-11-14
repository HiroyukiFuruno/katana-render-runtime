use super::{focus_candidates, next_candidate};
use crate::renderer::backends::html_document::HtmlDocumentNode;

const ROOT_ID: u64 = 1;
const NATURAL_ID: u64 = 2;
const LINK_ID: u64 = 3;
const FIRST_ID: u64 = 4;
const NEGATIVE_ID: u64 = 5;
const DISABLED_ID: u64 = 6;
const FIELDSET_ID: u64 = 7;
const LEGEND_ID: u64 = 8;
const LEGEND_BUTTON_ID: u64 = 9;
const FIELDSET_BUTTON_ID: u64 = 10;
const HIDDEN_ID: u64 = 11;
const HIDDEN_BUTTON_ID: u64 = 12;
const DETAILS_ID: u64 = 13;
const SUMMARY_ID: u64 = 14;
const SUMMARY_BUTTON_ID: u64 = 15;
const CLOSED_DETAILS_BUTTON_ID: u64 = 16;
const EDITABLE_ID: u64 = 17;
const NESTED_EDITABLE_ID: u64 = 18;
const EDITABLE_BUTTON_ID: u64 = 19;
const VISIBILITY_HIDDEN_ID: u64 = 20;
const VISIBILITY_HIDDEN_CHILD_ID: u64 = 21;
const VISIBILITY_VISIBLE_CHILD_ID: u64 = 22;
const FIRST_TAB_INDEX: &str = "1";
const SECOND_TAB_INDEX: &str = "2";
const NEGATIVE_TAB_INDEX: &str = "-1";

#[test]
fn tab_navigation_handles_empty_missing_middle_and_wraparound_focus() {
    let order = [FIRST_ID, LINK_ID, NATURAL_ID];
    for (focused, backwards, expected) in [
        (None, false, Some(FIRST_ID)),
        (None, true, Some(NATURAL_ID)),
        (Some(u64::MAX), false, Some(FIRST_ID)),
        (Some(u64::MAX), true, Some(NATURAL_ID)),
        (Some(LINK_ID), false, Some(NATURAL_ID)),
        (Some(LINK_ID), true, Some(FIRST_ID)),
        (Some(NATURAL_ID), false, Some(FIRST_ID)),
        (Some(FIRST_ID), true, Some(NATURAL_ID)),
    ] {
        assert_eq!(next_candidate(&order, focused, backwards), expected);
    }
    for backwards in [false, true] {
        assert_eq!(next_candidate(&[], Some(FIRST_ID), backwards), None);
    }
}

#[test]
fn tab_order_sorts_positive_values_and_skips_unavailable_nodes() {
    let (ordered, pointer_focusable) = focus_candidates(&fixture());
    assert_eq!(ordered, expected_order());
    assert!(pointer_focusable.contains(&NEGATIVE_ID));
    assert_next_candidates(&ordered);
}

#[test]
fn visibility_hidden_is_inherited_unless_a_descendant_is_visible() {
    let nodes = vec![element(
        ROOT_ID,
        "main",
        vec![],
        vec![element(
            VISIBILITY_HIDDEN_ID,
            "div",
            vec![("style", "visibility: hidden")],
            vec![
                element(VISIBILITY_HIDDEN_CHILD_ID, "button", vec![], vec![]),
                element(
                    VISIBILITY_VISIBLE_CHILD_ID,
                    "button",
                    vec![("style", "visibility: visible")],
                    vec![],
                ),
            ],
        )],
    )];

    let (ordered, _) = focus_candidates(&nodes);
    assert_eq!(ordered, vec![VISIBILITY_VISIBLE_CHILD_ID]);
}

fn assert_next_candidates(ordered: &[u64]) {
    assert_eq!(next_candidate(ordered, None, false), Some(FIRST_ID));
    assert_eq!(
        next_candidate(ordered, Some(EDITABLE_BUTTON_ID), false),
        Some(FIRST_ID)
    );
    assert_eq!(
        next_candidate(ordered, Some(FIRST_ID), true),
        Some(EDITABLE_BUTTON_ID)
    );
}

fn expected_order() -> Vec<u64> {
    vec![
        FIRST_ID,
        LINK_ID,
        NATURAL_ID,
        LEGEND_BUTTON_ID,
        SUMMARY_ID,
        SUMMARY_BUTTON_ID,
        EDITABLE_ID,
        EDITABLE_BUTTON_ID,
    ]
}

fn fixture() -> Vec<HtmlDocumentNode> {
    vec![element(ROOT_ID, "main", vec![], fixture_children())]
}

fn fixture_children() -> Vec<HtmlDocumentNode> {
    vec![
        element(NATURAL_ID, "button", vec![], vec![]),
        element(
            LINK_ID,
            "a",
            vec![("href", "#x"), ("tabindex", SECOND_TAB_INDEX)],
            vec![],
        ),
        element(
            FIRST_ID,
            "button",
            vec![("tabindex", FIRST_TAB_INDEX)],
            vec![],
        ),
        element(
            NEGATIVE_ID,
            "button",
            vec![("tabindex", NEGATIVE_TAB_INDEX)],
            vec![],
        ),
        element(DISABLED_ID, "button", vec![("disabled", "")], vec![]),
        disabled_fieldset(),
        hidden_subtree(),
        details(),
        editable_region(),
    ]
}

fn disabled_fieldset() -> HtmlDocumentNode {
    element(
        FIELDSET_ID,
        "fieldset",
        vec![("disabled", "")],
        vec![
            element(
                LEGEND_ID,
                "legend",
                vec![],
                vec![element(LEGEND_BUTTON_ID, "button", vec![], vec![])],
            ),
            element(FIELDSET_BUTTON_ID, "button", vec![], vec![]),
        ],
    )
}

fn hidden_subtree() -> HtmlDocumentNode {
    element(
        HIDDEN_ID,
        "div",
        vec![("hidden", "")],
        vec![element(HIDDEN_BUTTON_ID, "button", vec![], vec![])],
    )
}

fn details() -> HtmlDocumentNode {
    element(
        DETAILS_ID,
        "details",
        vec![],
        vec![
            element(
                SUMMARY_ID,
                "summary",
                vec![],
                vec![element(SUMMARY_BUTTON_ID, "button", vec![], vec![])],
            ),
            element(CLOSED_DETAILS_BUTTON_ID, "button", vec![], vec![]),
        ],
    )
}

fn editable_region() -> HtmlDocumentNode {
    element(
        EDITABLE_ID,
        "div",
        vec![("contenteditable", "true")],
        vec![
            element(
                NESTED_EDITABLE_ID,
                "div",
                vec![("contenteditable", "true")],
                vec![],
            ),
            element(EDITABLE_BUTTON_ID, "button", vec![], vec![]),
        ],
    )
}

fn element(
    node_id: u64,
    tag: &str,
    attributes: Vec<(&str, &str)>,
    children: Vec<HtmlDocumentNode>,
) -> HtmlDocumentNode {
    HtmlDocumentNode::Element {
        node_id,
        tag: tag.to_string(),
        attributes: attributes
            .into_iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
        children,
    }
}
