use super::{HtmlDocument, insert_target};

#[test]
fn adjacent_content_cases_preserve_sibling_boundaries() -> Result<(), String> {
    let cases = [
        AdjacentContentCase {
            kind: AdjacentContentKind::Html,
            before_position: " BEFOREBEGIN ",
            after_position: " AFTEREND ",
            before: "<i>head</i>",
            after: "<i>tail</i>",
            expected: "<i>head</i><p id=\"target\">body</p><i>tail</i>",
        },
        AdjacentContentCase {
            kind: AdjacentContentKind::Text,
            before_position: " BEFOREBEGIN ",
            after_position: " AFTEREND ",
            before: "<head>",
            after: "<tail>",
            expected: "&lt;head&gt;<p id=\"target\">body</p>&lt;tail&gt;",
        },
    ];
    for case in cases {
        run_adjacent_content_case(case)?;
    }
    Ok(())
}

fn run_adjacent_content_case(case: AdjacentContentCase) -> Result<(), String> {
    let mut document = HtmlDocument::parse("<main id=parent><p id=target>body</p></main>");
    let target = document
        .get_element_by_id("target")
        .ok_or_else(|| "target element was not indexed".to_string())?;
    let parent = document
        .get_element_by_id("parent")
        .ok_or_else(|| "parent element was not indexed".to_string())?;
    case.insert(&mut document, target, case.before_position, case.before)?;
    case.insert(&mut document, target, case.after_position, case.after)?;
    assert_eq!(
        document.inner_html(parent)?,
        case.expected,
        "{:?}",
        case.kind
    );
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum AdjacentContentKind {
    Html,
    Text,
}

struct AdjacentContentCase {
    kind: AdjacentContentKind,
    before_position: &'static str,
    after_position: &'static str,
    before: &'static str,
    after: &'static str,
    expected: &'static str,
}

impl AdjacentContentCase {
    fn insert(
        &self,
        document: &mut HtmlDocument,
        target: u64,
        position: &str,
        value: &str,
    ) -> Result<(), String> {
        match self.kind {
            AdjacentContentKind::Html => document.insert_adjacent_html(target, position, value),
            AdjacentContentKind::Text => document.insert_adjacent_text(target, position, value),
        }
    }
}

#[test]
fn insert_target_reports_after_end_index_for_attached_target() -> Result<(), String> {
    let mut document = HtmlDocument::parse("<main><p id=target>body</p></main>");
    let target = document
        .get_element_by_id("target")
        .ok_or_else(|| "target element was not indexed".to_string())?;
    let target_node = document.node(target)?;
    let parent_link = target_node.parent.take();
    let parent = parent_link
        .as_ref()
        .and_then(|parent| parent.upgrade())
        .ok_or_else(|| "target parent was not available".to_string())?;
    target_node.parent.set(parent_link);
    let index = insert_target(&target_node, "afterend")?.1;

    assert_eq!(index, parent.children.borrow().len());
    Ok(())
}

#[test]
fn set_inner_html_clears_select_empty_override_for_consumer_value() -> Result<(), String> {
    let mut document =
        HtmlDocument::parse("<select id=choice><option value=old selected>Old</option></select>");
    let select = document
        .get_element_by_id("choice")
        .ok_or_else(|| "select element was not indexed".to_string())?;

    document.set_select_value(select, "missing")?;
    assert_eq!(document.form_value(select)?, Some(String::new()));

    document.set_inner_html(select, "<option value=new>New</option>")?;

    assert_eq!(document.form_value(select)?, Some("new".to_string()));
    Ok(())
}
