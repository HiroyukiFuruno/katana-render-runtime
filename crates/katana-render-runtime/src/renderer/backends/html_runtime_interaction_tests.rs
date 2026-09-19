use super::html_runtime::StaticHtmlRuntimeSession;
use super::{HtmlNodeId, HtmlRenderInput, HtmlRenderer, HtmlRuntimeEvent, StaticHtmlRuntime};

type TestResult<T = ()> = Result<T, String>;

#[test]
fn dispatches_click_listeners_and_honors_prevent_default() -> TestResult {
    let mut session = StaticHtmlRuntime
        .start(
            "<a id=link href=\"/next\">Open</a><script>const link = document.getElementById('link'); link.addEventListener('click', (event) => { event.preventDefault(); link.textContent = 'Handled'; link.style.color = 'green'; });</script>",
        )
        .map_err(|error| format!("HTML runtime session must start: {error}"))?;
    let link = session
        .node_for_element_id("link")
        .ok_or_else(|| "link must have a stable node id".to_string())?;

    let dispatch = session
        .dispatch(HtmlRuntimeEvent::Click { target: link })
        .map_err(|error| format!("click must dispatch: {error}"))?;

    assert!(dispatch.content.contains("Handled"), "{}", dispatch.content);
    assert!(
        dispatch.content.contains(r#"style="color: green""#),
        "{}",
        dispatch.content
    );
    assert_eq!(dispatch.navigation, None);
    Ok(())
}

#[test]
fn dispatches_focus_keyboard_change_and_blur_event_data() -> TestResult {
    let mut session = StaticHtmlRuntime
        .start(
            "<input id=field><p id=status></p><script>const field = document.getElementById('field'); const status = document.getElementById('status'); for (const type of ['focus', 'keydown', 'keyup', 'change', 'blur']) field.addEventListener(type, (event) => { status.textContent += `${event.type}${event.key ? ':' + event.key : ''}|`; });</script>",
        )
        .map_err(|error| format!("HTML runtime session must start: {error}"))?;
    let field = session
        .node_for_element_id("field")
        .ok_or_else(|| "field must have a stable node id".to_string())?;
    dispatch_focus_events(&mut session, field)?;

    let snapshot = session
        .snapshot()
        .map_err(|error| format!("snapshot must render: {error}"))?;
    assert!(
        snapshot.contains("focus|keydown:Enter|keyup:Enter|change|blur|"),
        "{snapshot}"
    );
    Ok(())
}

fn dispatch_focus_events(session: &mut StaticHtmlRuntimeSession, field: HtmlNodeId) -> TestResult {
    for event in [
        HtmlRuntimeEvent::Focus { target: field },
        HtmlRuntimeEvent::KeyDown {
            target: field,
            key: "Enter".to_string(),
        },
        HtmlRuntimeEvent::KeyUp {
            target: field,
            key: "Enter".to_string(),
        },
        HtmlRuntimeEvent::Change { target: field },
        HtmlRuntimeEvent::Blur { target: field },
    ] {
        session
            .dispatch(event)
            .map_err(|error| format!("event must dispatch: {error}"))?;
    }
    Ok(())
}

#[test]
fn dispatches_inline_onclick_and_returns_link_navigation_intent() -> TestResult {
    let mut session = StaticHtmlRuntime
        .start("<a id=link href=\"/next\" onclick=\"this.textContent = 'Clicked'\">Open</a>")
        .map_err(|error| format!("HTML runtime session must start: {error}"))?;
    let link = session
        .node_for_element_id("link")
        .ok_or_else(|| "link must have a stable node id".to_string())?;

    let dispatch = session
        .dispatch(HtmlRuntimeEvent::Click { target: link })
        .map_err(|error| format!("click must dispatch: {error}"))?;

    assert!(dispatch.content.contains("Clicked"), "{}", dispatch.content);
    assert_eq!(
        dispatch.navigation.map(|intent| intent.href),
        Some("/next".to_string())
    );
    Ok(())
}

#[test]
fn evaluates_create_element_and_append_child_bridge() -> TestResult {
    let output = render(
        "<div id=host></div><script>const child = document.createElement('span'); child.textContent = 'Child'; document.getElementById('host').appendChild(child);</script>",
    )?;

    assert!(output.contains("<span>Child</span>"), "{output}");
    Ok(())
}

#[test]
fn evaluates_attribute_removal_and_style_mutation_bridge() -> TestResult {
    let output = render(
        "<p id=target data-state=ready style=\"color: red\">Initial</p><script>const target = document.getElementById('target'); target.removeAttribute('data-state'); target.style.backgroundColor = '#bbf7d0'; target.style.color = 'green'; target.textContent = target.style.color;</script>",
    )?;

    assert!(!output.contains("data-state"), "{output}");
    assert!(output.contains("background-color: #bbf7d0"), "{output}");
    assert!(output.contains(">green</p>"), "{output}");
    Ok(())
}

#[test]
fn dispatches_document_and_window_lifecycle_events_in_browser_order() -> TestResult {
    let output = render(
        r#"<p id=status></p><script>
const status = document.getElementById('status');
const events = [`script:${document.readyState}`];
document.addEventListener('readystatechange', () => events.push(`state:${document.readyState}`));
document.addEventListener('DOMContentLoaded', function (event) {
  events.push(`dom:${document.readyState}:${event.type}:${event.target === document}:${this === document}`);
  Promise.resolve().then(() => events.push(`microtask:${document.readyState}`));
});
window.addEventListener('load', function (event) {
  events.push(`load:${document.readyState}:${event.target === window}:${this === window}`);
  status.textContent = events.join('|');
});
</script>"#,
    )?;

    assert!(
        output.contains(
            "script:loading|state:interactive|dom:interactive:DOMContentLoaded:true:true|microtask:interactive|state:complete|load:complete:true:true"
        ),
        "{output}"
    );
    Ok(())
}

#[test]
fn document_event_target_honors_once_remove_and_handle_event() -> TestResult {
    let output = render(
        r#"<p id=status></p><script>
const status = document.getElementById('status');
const removed = () => { status.textContent += 'removed|'; };
document.addEventListener('custom', removed);
document.removeEventListener('custom', removed);
document.addEventListener('custom', () => { status.textContent += 'once|'; }, { once: true });
document.dispatchEvent(new Event('custom'));
document.dispatchEvent(new Event('custom'));
document.addEventListener('DOMContentLoaded', { handleEvent(event) { status.textContent += `${event.type}|`; } });
</script>"#,
    )?;

    assert!(output.contains(">once|DOMContentLoaded|</p>"), "{output}");
    assert!(!output.contains("removed|"), "{output}");
    Ok(())
}

#[test]
fn lifecycle_handler_properties_receive_browser_state_and_event_target() -> TestResult {
    let output = render(
        r#"<p id=status></p><script>
const status = document.getElementById('status');
document.onreadystatechange = (event) => {
  status.textContent += `${document.readyState}:${event.target === document}|`;
};
window.onload = function (event) {
  status.textContent += `${document.readyState}:${event.target === window}:${this === window}`;
};
const nullableOptions = new Event('custom', null);
status.dataset.nullable = `${nullableOptions.bubbles}:${nullableOptions.cancelable}`;
</script>"#,
    )?;

    assert!(output.contains("data-nullable=\"false:false\""), "{output}");
    assert!(
        output.contains(">interactive:true|complete:true|complete:true:true</p>"),
        "{output}"
    );
    Ok(())
}

#[test]
fn reports_document_lifecycle_listener_failures() {
    assert!(matches!(
        render(
            "<script>document.addEventListener('DOMContentLoaded', () => { throw new Error('lifecycle failed'); });</script>"
        ),
        Err(message)
            if message.contains("JavaScript exception")
                && message.contains("lifecycle failed")
                && message.contains("inline-script:1:")
    ));
}

#[test]
fn query_selector_uses_the_css_compound_and_ancestry_engine() -> TestResult {
    let output = render(
        r#"<main><section class="card" data-state="ready"><p class="message emphasis">Initial</p></section></main><script>document.querySelector('main > section.card[data-state=ready] p.message.emphasis').textContent = 'Matched';</script>"#,
    )?;

    assert!(output.contains(">Matched</p>"), "{output}");
    Ok(())
}

#[test]
fn query_selector_all_returns_each_matching_dom_element() -> TestResult {
    let output = render(
        r#"<main><p class="message">One</p><p class="message">Two</p></main><p id=count></p><script>const matches = document.querySelectorAll('main > p.message'); matches.forEach((item, index) => { item.textContent = String(index + 1); }); document.getElementById('count').textContent = String(matches.length);</script>"#,
    )?;

    assert!(output.contains(r#"<p class="message">1</p>"#), "{output}");
    assert!(output.contains(r#"<p class="message">2</p>"#), "{output}");
    assert!(output.contains("<p id=\"count\">2</p>"), "{output}");
    Ok(())
}

#[test]
fn url_attribute_selectors_work_through_query_selector_and_query_selector_all() -> TestResult {
    let output = render(
        r#"<main><a href="https://example.com">One</a><a href="https://example.com">Two</a><a href="https://example.net">Other</a></main><p id=count></p><script>const selector = 'a[href="https://example.com"]'; document.querySelector(selector).textContent = 'Matched'; const matches = document.querySelectorAll(selector); matches.forEach((item) => { item.setAttribute('data-match', 'yes'); }); document.getElementById('count').textContent = String(matches.length);</script>"#,
    )?;

    assert!(
        output.contains(r#"<a href="https://example.com" data-match="yes">Matched</a>"#),
        "{output}"
    );
    assert!(
        output.contains(r#"<a href="https://example.com" data-match="yes">Two</a>"#),
        "{output}"
    );
    assert!(output.contains("<p id=\"count\">2</p>"), "{output}");
    Ok(())
}

#[test]
fn element_matches_and_closest_use_the_css_selector_engine() -> TestResult {
    let output = render(
        r#"<main class="deck"><section class="slide active"><span id="target" class="label">Text</span></section></main><p id="result"></p><script>const target = document.getElementById('target'); const slide = target.closest('main.deck > section.slide.active'); const detached = document.createElement('div'); detached.className = 'detached'; document.getElementById('result').textContent = [target.matches('span#target.label'), slide === document.querySelector('.slide'), target.closest('.missing') === null, detached.matches('.detached'), detached.closest('.detached') === detached].join(':');</script>"#,
    )?;

    assert!(output.contains(">true:true:true:true:true</p>"), "{output}");
    Ok(())
}

fn render(source: &str) -> TestResult<String> {
    HtmlRenderer
        .render(&HtmlRenderInput {
            source: source.to_string(),
        })
        .map(|output| output.content)
        .map_err(|error| format!("HTML runtime must render test fixture: {error}"))
}
