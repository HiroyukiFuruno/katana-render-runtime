use super::super::super::html_browser::HtmlBrowserInput;
use super::support::{TestResult, click_element, start};

const TAB: &str = "Tab";
const SHIFT: &str = "Shift";

#[test]
fn pointer_to_keyboard_refocus_renders_visible_focus_for_each_control() -> TestResult {
    for markup in [
        "<button id=target>Focus</button>",
        "<a id=target href='#target'>Focus</a>",
    ] {
        assert_refocus_visibility(markup)?;
    }
    Ok(())
}

fn assert_refocus_visibility(markup: &str) -> TestResult {
    let html = format!(
        "{markup}<p id=events></p><script>const target = document.getElementById('target'); const events = document.getElementById('events'); for (const type of ['focus','blur']) target.addEventListener(type, () => events.textContent += type + '|');</script>"
    );
    let mut session = start(&html)?;
    let node = element_id(&mut session, "target")?;
    click_element(&mut session, "target")?;
    assert_eq!(session.focused_node, Some(node));
    assert!(!session.keyboard_modality && !session.focus_visible);
    let generation = session.generation;
    press_key(&mut session, TAB)?;
    assert_eq!(session.focused_node, Some(node));
    assert!(session.keyboard_modality && session.focus_visible);
    assert_eq!(session.generation, generation + 1);
    let snapshot = session
        .runtime
        .snapshot()
        .map_err(|error| error.to_string())?;
    assert!(snapshot.contains(">focus|</p>"), "{snapshot}");
    Ok(())
}

#[test]
fn tab_order_and_shift_tab_update_focus_and_active_element() -> TestResult {
    let mut session = start(tab_order_fixture())?;
    let first = element_id(&mut session, "first")?;
    let later = element_id(&mut session, "later")?;
    let natural = element_id(&mut session, "natural")?;

    press_key(&mut session, TAB)?;
    assert_eq!(session.focused_node, Some(first));
    press_key(&mut session, TAB)?;
    assert_eq!(session.focused_node, Some(later));
    press_key(&mut session, TAB)?;
    assert_eq!(session.focused_node, Some(natural));
    press_key(&mut session, TAB)?;
    assert_eq!(session.focused_node, Some(first));
    press_key(&mut session, SHIFT)?;
    press_key(&mut session, TAB)?;
    assert_eq!(session.focused_node, Some(natural));
    release_key(&mut session, SHIFT)?;

    let snapshot = session
        .runtime
        .snapshot()
        .map_err(|error| error.to_string())?;
    assert!(
        snapshot.contains("first:first|later:later|natural:natural|"),
        "{snapshot}"
    );
    Ok(())
}

fn tab_order_fixture() -> &'static str {
    r#"<button id="later" tabindex="2">Later</button>
<button id="first" tabindex="1">First</button>
<button id="natural">Natural</button><p id="events"></p>
<script>document.addEventListener('focus', event => {
  document.getElementById('events').textContent +=
    `${event.target.id}:${document.activeElement.id}|`;
}, true);</script>"#
}

#[test]
fn cancelled_tab_keeps_focus_and_pointer_focus_uses_control_heuristic() -> TestResult {
    let html = r#"<button id="button">Button</button><input id="field">
<script>document.addEventListener('keydown', event => {
  if (event.key === 'Tab') event.preventDefault();
});</script>"#;
    let mut session = start(html)?;

    press_key(&mut session, TAB)?;
    assert_eq!(session.focused_node, None);
    click_element(&mut session, "button")?;
    assert!(!session.focus_visible);
    click_element(&mut session, "field")?;
    assert!(session.focus_visible);
    Ok(())
}

#[test]
fn tab_refreshes_candidates_after_keydown_and_renders_one_frame() -> TestResult {
    let html = r#"<button id="skip">Skip</button><button id="next">Next</button>
<script>document.addEventListener('keydown', event => {
  if (event.key === 'Tab') document.getElementById('skip').setAttribute('disabled', '');
});</script>"#;
    let mut session = start(html)?;
    let next = element_id(&mut session, "next")?;
    let generation = session.generation;

    press_key(&mut session, TAB)?;

    assert_eq!(session.generation, generation + 1);
    assert_eq!(session.focused_node, Some(next));
    Ok(())
}

#[test]
fn tab_dispatches_change_blur_focus_before_its_single_frame() -> TestResult {
    let mut session = start(tab_event_order_fixture())?;
    let field = element_id(&mut session, "field")?;
    let next = element_id(&mut session, "next")?;
    click_element(&mut session, "field")?;
    session.dirty_inputs.insert(field);
    let generation = session.generation;

    press_key(&mut session, TAB)?;

    assert_eq!(session.generation, generation + 1);
    assert_eq!(session.focused_node, Some(next));
    let snapshot = session
        .runtime
        .snapshot()
        .map_err(|error| error.to_string())?;
    assert!(
        snapshot.contains("keydown|change:field|blur:page|focus:next"),
        "{snapshot}"
    );
    Ok(())
}

fn tab_event_order_fixture() -> &'static str {
    r#"<body id="page"><input id="field"><button id="next">Next</button>
<p id="events"></p><script>
const events = document.getElementById('events');
document.addEventListener('keydown', () => events.textContent += 'keydown|');
document.getElementById('field').addEventListener('change', () =>
  events.textContent += `change:${document.activeElement.id}|`);
document.getElementById('field').addEventListener('blur', () =>
  events.textContent += `blur:${document.activeElement.id}|`);
document.getElementById('next').addEventListener('focus', () =>
  events.textContent += `focus:${document.activeElement.id}|`);
</script></body>"#
}

fn element_id(session: &mut super::super::HtmlInteractiveSession, id: &str) -> TestResult<u64> {
    session
        .runtime
        .node_for_element_id(id)
        .map(|node| node.0)
        .ok_or_else(|| format!("missing focus element #{id}"))
}

fn press_key(session: &mut super::super::HtmlInteractiveSession, key: &str) -> TestResult {
    session
        .dispatch_input(HtmlBrowserInput::KeyDown {
            key: key.to_string(),
        })
        .map_err(|error| error.to_string())
}

fn release_key(session: &mut super::super::HtmlInteractiveSession, key: &str) -> TestResult {
    session
        .dispatch_input(HtmlBrowserInput::KeyUp {
            key: key.to_string(),
        })
        .map_err(|error| error.to_string())
}
