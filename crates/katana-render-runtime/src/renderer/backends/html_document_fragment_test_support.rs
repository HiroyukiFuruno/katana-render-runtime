use super::HtmlDocument;

#[derive(Clone, Copy)]
pub(super) enum EmptyHtmlMutation {
    SetInnerHtml,
    InsertAdjacentHtml,
}

pub(super) const EMPTY_HTML_MUTATION_CASES: [(&str, EmptyHtmlMutation); 2] = [
    ("set_inner_html", EmptyHtmlMutation::SetInnerHtml),
    (
        "insert_adjacent_html",
        EmptyHtmlMutation::InsertAdjacentHtml,
    ),
];

pub(super) fn run_empty_html_mutation(
    document: &mut HtmlDocument,
    target: u64,
    mutation: EmptyHtmlMutation,
) -> Result<(), String> {
    match mutation {
        EmptyHtmlMutation::SetInnerHtml => document.set_inner_html(target, ""),
        EmptyHtmlMutation::InsertAdjacentHtml => {
            document.insert_adjacent_html(target, "beforeend", "")
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum FragmentMutation {
    SetInnerHtml,
    InsertAdjacentHtml,
    InsertAdjacentText,
}

pub(super) const FRAGMENT_MUTATIONS: [(&str, FragmentMutation); 3] = [
    ("set_inner_html", FragmentMutation::SetInnerHtml),
    ("insert_adjacent_html", FragmentMutation::InsertAdjacentHtml),
    ("insert_adjacent_text", FragmentMutation::InsertAdjacentText),
];

type AdjacentInsert = fn(&mut HtmlDocument, u64, &str, &str) -> Result<(), String>;
const ADJACENT_POSITION_COUNT: usize = 4;

pub(super) struct AdjacentContentCase {
    kind: &'static str,
    insert: AdjacentInsert,
    values: [&'static str; ADJACENT_POSITION_COUNT],
    expected: &'static str,
}

pub(super) const ADJACENT_POSITIONS: [&str; ADJACENT_POSITION_COUNT] =
    ["beforebegin", "afterbegin", "beforeend", "afterend"];
pub(super) const ADJACENT_CONTENT_CASES: [AdjacentContentCase; 2] = [
    AdjacentContentCase {
        kind: "HTML",
        insert: HtmlDocument::insert_adjacent_html,
        values: [
            "<i>before</i>",
            "<b>first</b>",
            "<b>last</b>",
            "<i>after</i>",
        ],
        expected: "<i>before</i><p id=\"target\"><b>first</b>Target<b>last</b></p><i>after</i>",
    },
    AdjacentContentCase {
        kind: "Text",
        insert: HtmlDocument::insert_adjacent_text,
        values: [
            "<outer-before>",
            "<inner-first>",
            "<inner-last>",
            "<outer-after>",
        ],
        expected: "&lt;outer-before&gt;<p id=\"target\">&lt;inner-first&gt;Target&lt;inner-last&gt;</p>&lt;outer-after&gt;",
    },
];

pub(super) fn assert_adjacent_content_case(
    case: AdjacentContentCase,
    positions: &[&str; ADJACENT_POSITION_COUNT],
) -> Result<(), String> {
    let mut document = HtmlDocument::parse("<main id=parent><p id=target>Target</p></main>");
    let target = document
        .get_element_by_id("target")
        .ok_or("target must exist")?;
    let parent = document
        .get_element_by_id("parent")
        .ok_or("parent must exist")?;
    for (position, value) in positions.iter().zip(case.values) {
        (case.insert)(&mut document, target, position, value)?;
    }
    assert_eq!(document.inner_html(parent)?, case.expected, "{}", case.kind);
    if case.kind == "Text" {
        assert!(matches!(
            document.insert_adjacent_text(1, "beforeend", "ignored"),
            Err(error) if error == "insertAdjacentText target is not an element"
        ));
    }
    Ok(())
}
