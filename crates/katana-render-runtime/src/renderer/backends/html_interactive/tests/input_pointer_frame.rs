use super::super::super::html_browser::HtmlBrowserInput;
use super::super::HtmlInteractiveSession;
use super::support::{TestResult, click_element, start};

#[test]
fn pointer_actions_render_one_frame_for_each_control_kind() -> TestResult {
    let html = r#"<button id="button">Button</button>
<input id="check" type="checkbox"><details id="details"><summary id="summary">More</summary><p>Body</p></details>
<input id="field"><p id="events"></p><script>
const events = document.getElementById('events');
document.getElementById('summary').addEventListener('click', () => events.textContent += 'click|');
document.getElementById('details').addEventListener('toggle', () => events.textContent += 'toggle|');
</script>"#;
    let mut session = start(html)?;

    for target in ["button", "check", "summary", "field"] {
        let generation = session.generation;
        click_element(&mut session, target)?;
        assert_eq!(session.generation, generation + 1, "#{target}");
    }
    assert!(session.focused_input.is_some());
    assert!(session.focus_visible);
    let state = session
        .runtime
        .observe_feature_state(
            "({events: document.getElementById('events').textContent, open: document.getElementById('details').open})",
        )
        .map_err(|error| error.to_string())?;
    assert_eq!(state["events"], "click|toggle|");
    assert_eq!(state["open"], true);
    Ok(())
}

#[test]
fn repeated_pointer_activation_keeps_same_focus_without_refiring_focus_events() -> TestResult {
    let html = r#"<button id="button">Button</button><p id="events"></p>
<script>document.addEventListener('focus', event => {
  document.getElementById('events').textContent += `focus:${event.target.id}|`;
}, true);
document.addEventListener('blur', event => {
  document.getElementById('events').textContent += `blur:${event.target.id}|`;
}, true);</script>"#;
    let mut session = start(html)?;
    click_element(&mut session, "button")?;
    let generation = session.generation;

    click_element(&mut session, "button")?;

    assert_eq!(session.generation, generation + 1);
    let events = session
        .runtime
        .observe_feature_state("document.getElementById('events').textContent")
        .map_err(|error| error.to_string())?;
    assert_eq!(events, "focus:button|");
    Ok(())
}

#[test]
fn pointer_transition_commits_dirty_input_before_focus_and_click_in_one_frame() -> TestResult {
    let html = r#"<input id="field"><button id="next">Next</button><p id="events"></p>
<script>
const events = document.getElementById('events');
document.getElementById('field').addEventListener('change', () => events.textContent += 'change|');
document.getElementById('field').addEventListener('blur', () => events.textContent += 'blur|');
document.getElementById('next').addEventListener('focus', () => events.textContent += 'focus|');
document.getElementById('next').addEventListener('click', () => events.textContent += 'click|');
</script>"#;
    let mut session = start(html)?;
    click_element(&mut session, "field")?;
    let field = element_id(&mut session, "field")?;
    session.dirty_inputs.insert(field);
    let generation = session.generation;

    click_element(&mut session, "next")?;

    assert_eq!(session.generation, generation + 1);
    let next = element_id(&mut session, "next")?;
    assert_eq!(session.focused_node, Some(next));
    let events = session
        .runtime
        .observe_feature_state("document.getElementById('events').textContent")
        .map_err(|error| error.to_string())?;
    assert_eq!(events, "change|blur|focus|click|");
    Ok(())
}

#[test]
fn pointer_click_listener_error_propagates_without_a_second_frame() -> TestResult {
    let html = r#"<button id="button">Button</button><script>
document.getElementById('button').addEventListener('click', () => {
  document.getElementById('button').textContent = 'Updated';
  throw new Error('click failed');
});</script>"#;
    let mut session = start(html)?;
    let generation = session.generation;
    let result = click_element(&mut session, "button");

    let error = result
        .err()
        .ok_or_else(|| "click error was not propagated".to_string())?;
    assert!(error.contains("click failed"), "{error}");
    assert_eq!(session.generation, generation);
    assert_eq!(session.pressed_target, None);
    let button_text = session
        .runtime
        .observe_feature_state("document.getElementById('button').textContent")
        .map_err(|error| error.to_string())?;
    assert_eq!(button_text, "Updated");
    Ok(())
}

#[test]
fn mismatched_pointer_release_does_not_render() -> TestResult {
    let mut session = start("<button id=first>First</button><button id=second>Second</button>")?;
    let first = element_id(&mut session, "first")?;
    let second = element_id(&mut session, "second")?;
    let target = session
        .hit_targets
        .iter()
        .find(|target| target.node_id == second)
        .cloned()
        .ok_or_else(|| "missing second button hit target".to_string())?;
    session.pressed_target = Some(first);
    let generation = session.generation;

    session
        .dispatch_input(HtmlBrowserInput::PointerUp {
            x: target.x + 1.0,
            y: target.y + 1.0 - session.scroll_geometry.scroll_y,
            button: 0,
        })
        .map_err(|error| error.to_string())?;

    assert_eq!(session.generation, generation);
    Ok(())
}

#[test]
fn blank_pointer_activation_blurs_once() -> TestResult {
    let mut session = start("<input id=field>")?;
    click_element(&mut session, "field")?;
    let generation = session.generation;

    session
        .dispatch_input(HtmlBrowserInput::PointerDown {
            x: 319.0,
            y: 239.0,
            button: 0,
        })
        .map_err(|error| error.to_string())?;
    session
        .dispatch_input(HtmlBrowserInput::PointerUp {
            x: 319.0,
            y: 239.0,
            button: 0,
        })
        .map_err(|error| error.to_string())?;

    assert_eq!(session.generation, generation + 1);
    assert_eq!(session.focused_node, None);
    Ok(())
}

fn element_id(session: &mut HtmlInteractiveSession, id: &str) -> TestResult<u64> {
    session
        .runtime
        .node_for_element_id(id)
        .map(|node| node.0)
        .ok_or_else(|| format!("missing focus element #{id}"))
}
