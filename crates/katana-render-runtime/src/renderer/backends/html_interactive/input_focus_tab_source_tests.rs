use super::super::super::{HtmlBrowserError, HtmlInteractiveSession};
use super::{focus_candidates, next_candidate};
use crate::{HtmlBrowserSource, HtmlBrowserViewport};

const VIEWPORT_WIDTH: u32 = 320;
const VIEWPORT_HEIGHT: u32 = 240;
const HTML: &str = "<button id=button tabindex=invalid>button</button><div id=editor contenteditable=plaintext-only>edit</div><details>prefix<p>before summary</p><summary id=summary>summary</summary>tail</details>";

fn open() -> Result<HtmlInteractiveSession, HtmlBrowserError> {
    let source = HtmlBrowserSource::new(HTML, "https://example.test/tab.html")?;
    let viewport = HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0)?;
    HtmlInteractiveSession::start(source, viewport)
}

fn expected_order(session: &mut HtmlInteractiveSession) -> Result<Vec<u64>, String> {
    ["button", "editor", "summary"]
        .map(|id| session.runtime.node_for_element_id(id).map(|node| node.0))
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| "missing source target".to_owned())
}

#[test]
fn source_tab_order_recovers_invalid_index_plaintext_editor_and_summary_among_text()
-> Result<(), String> {
    let mut session = open().map_err(|error| error.to_string())?;
    let nodes = session
        .runtime
        .interactive_nodes()
        .map_err(|error| error.to_string())?;
    let expected = expected_order(&mut session)?;
    let (ordered, pointer_focusable) = focus_candidates(&nodes);
    assert_eq!(ordered, expected);
    assert_eq!(pointer_focusable.len(), expected.len());
    assert_eq!(
        next_candidate(&ordered, None, true),
        expected.last().copied()
    );
    assert_eq!(
        next_candidate(&ordered, Some(u64::MAX), true),
        expected.last().copied()
    );
    Ok(())
}
