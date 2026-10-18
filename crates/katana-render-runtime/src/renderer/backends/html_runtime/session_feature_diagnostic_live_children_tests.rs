use super::super::{StaticHtmlRuntime, test_error};
use crate::renderer::backends::html_browser::HtmlBrowserSource;
use serde_json::json;

const LIVE_CHILD_COLLECTION_EXPRESSION: &str = r#"(() => {
  const parent = document.getElementById('parent');
  const other = document.getElementById('other');
  const children = parent.children;
  const iterator = children[Symbol.iterator]();
  const first = iterator.next().value.id;
  parent.firstElementChild.remove();
  const nextAfterRemove = iterator.next().value.id;
  const appended = document.createElement('b');
  appended.id = 'four';
  parent.appendChild(appended);
  const nextAfterAppend = iterator.next().value.id;
  const moved = parent.querySelector('#two');
  other.appendChild(moved);
  const reparentedChildren = Array.from(children).map((child) => child.id);
  parent.innerHTML = '<em id=replacement name=old></em>';
  const sameCollection = children === parent.children;
  const replacement = children.namedItem('old');
  const aliasBeforeAttributeChange = children.replacement === replacement;
  replacement.id = 'newId';
  replacement.setAttribute('name', 'newName');
  return {
    sameCollection,
    first,
    nextAfterRemove,
    nextAfterAppend,
    reparentedChildren,
    movedParent: other.children[0].id,
    replacementLength: children.length,
    aliasBeforeAttributeChange,
    oldNameRemoved: children.namedItem('old') === null,
    namedById: children.namedItem('newId') === replacement,
    namedByName: children.namedItem('newName') === replacement,
    aliasById: children.newId === replacement,
    aliasByName: children.newName === replacement,
  };
})()"#;

#[test]
fn v8_children_collection_tracks_live_rcdom_mutations() -> Result<(), String> {
    let source = HtmlBrowserSource::new(
        "<main id=parent><i id=one></i><b id=two name=old></b><u id=three></u></main><aside id=other></aside>",
        "https://example.test/live-children.html",
    )
    .map_err(test_error)?;
    let mut session = StaticHtmlRuntime
        .start_interactive(&source)
        .map_err(test_error)?;
    let observed = session
        .observe_feature_state(LIVE_CHILD_COLLECTION_EXPRESSION)
        .map_err(test_error)?;
    assert_live_child_collection_state(observed);
    Ok(())
}

fn assert_live_child_collection_state(observed: serde_json::Value) {
    assert_eq!(
        observed,
        json!({
            "sameCollection": true,
            "first": "one",
            "nextAfterRemove": "three",
            "nextAfterAppend": "four",
            "reparentedChildren": ["three", "four"],
            "movedParent": "two",
            "replacementLength": 1,
            "aliasBeforeAttributeChange": true,
            "oldNameRemoved": true,
            "namedById": true,
            "namedByName": true,
            "aliasById": true,
            "aliasByName": true,
        })
    );
}
