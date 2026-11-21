use super::{HtmlRenderInput, HtmlRenderer};

type TestResult<T = ()> = Result<T, String>;

#[path = "html_runtime_lifecycle_body_tests.rs"]
mod body;

pub(super) const LATER_BODY_ONLOAD_CASES: &[(&str, &str, &str)] = &[
    (
        r#""#,
        r#"<body onload="document.getElementById('status').textContent = 'second'">"#,
        "first",
    ),
    (
        r#"window.onload = () => document.getElementById('status').textContent = 'author';"#,
        r#"<body onload="document.getElementById('status').textContent = 'first'">"#,
        "author",
    ),
    (
        r#"document.body.removeAttribute('onload');"#,
        r#"<body onload="document.getElementById('status').textContent = 'second'"><script>document.body.removeAttribute('onload');</script><body onload="document.getElementById('status').textContent = 'third'">"#,
        "third",
    ),
    (
        r#"document.body.removeAttribute('onload');"#,
        r#"<template><body onload="document.getElementById('status').textContent = 'second'"></template><select><body onload="document.getElementById('status').textContent = 'second'"></select>"#,
        "Waiting",
    ),
    (
        r#"document.body.removeAttribute('onload'); document.body.getAttribute = () => 'spoof'; document.body.setAttribute = () => {}; Object.defineProperty(document, 'body', { get: () => null }); document.querySelector = () => null; globalThis.__krr_dom = () => null;"#,
        r#"<body onload="document.getElementById('status').textContent = 'second'">"#,
        "second",
    ),
    (
        r#"const savedFunction = Function; const savedString = String; const savedApply = Reflect.apply; const statusCaptured = document.getElementById('status'); document.body.removeAttribute('onload'); globalThis.__krrInstallStaticBodyLoadHandler = () => {}; globalThis.Function = () => null; globalThis.String = () => 'spoof'; Reflect.apply = () => null;"#,
        r#"<body onload="statusCaptured.textContent = 'second'"><script>globalThis.Function = savedFunction; globalThis.String = savedString; Reflect.apply = savedApply;</script>"#,
        "second",
    ),
    (
        r#"const savedMapGet = Map.prototype.get; const savedMapSet = Map.prototype.set; const savedMapDelete = Map.prototype.delete; const savedWeakGet = WeakMap.prototype.get; const savedWeakSet = WeakMap.prototype.set; const statusCaptured = document.getElementById('status'); document.body.removeAttribute('onload'); Map.prototype.get = () => undefined; Map.prototype.set = () => {}; Map.prototype.delete = () => {}; WeakMap.prototype.get = () => undefined; WeakMap.prototype.set = () => {};"#,
        r#"<body onload="statusCaptured.textContent = 'second'"><script>Map.prototype.get = savedMapGet; Map.prototype.set = savedMapSet; Map.prototype.delete = savedMapDelete; WeakMap.prototype.get = savedWeakGet; WeakMap.prototype.set = savedWeakSet;</script>"#,
        "second",
    ),
    (
        r#"document.body.removeAttribute('onload'); window.authorCalls = 0; globalThis.__krrInstallStaticBodyLoadHandler = () => { window.authorCalls += 1; };"#,
        r#"<body onload="document.getElementById('status').textContent = `second:${window.authorCalls}`"><script>__krrInstallStaticBodyLoadHandler('author-source', true);</script>"#,
        "second:1",
    ),
];

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
fn dispatches_resource_events_before_the_complete_ready_state() -> TestResult {
    let output = render(
        r#"<p id=status></p><iframe data-krr-local-frame onload="document.getElementById('status').textContent += `frame:${document.readyState}|`"></iframe><img src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==" onload="document.getElementById('status').textContent += `image:${document.readyState}|`"><img src="data:image/png;base64,!" onerror="document.getElementById('status').textContent += `error:${document.readyState}|`"><script>document.addEventListener('readystatechange', () => { if (document.readyState === 'complete') document.getElementById('status').textContent += 'complete|'; });</script>"#,
    )?;

    assert!(
        output.contains(">frame:interactive|image:interactive|error:interactive|complete|</p>"),
        "{output}"
    );
    Ok(())
}

#[test]
fn iframe_load_handler_failure_does_not_interrupt_lifecycle_dispatch() -> TestResult {
    let output = render(
        r#"<p id=status></p><iframe data-krr-local-frame onload="throw new Error('frame failed')"></iframe><iframe data-krr-local-frame onload="document.getElementById('status').textContent += 'later-frame|' "></iframe><img src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==" onload="document.getElementById('status').textContent += 'image|' "><script>
document.addEventListener('readystatechange', () => { if (document.readyState === 'complete') document.getElementById('status').textContent += 'complete|'; });
window.addEventListener('load', () => { document.getElementById('status').textContent += 'window-load|'; });
</script>"#,
    )?;

    assert!(
        output.contains(">later-frame|image|complete|window-load|</p>"),
        "{output}"
    );
    Ok(())
}

#[test]
fn dispatches_load_for_local_iframes_added_by_a_lifecycle_handler() -> TestResult {
    let output = render(
        r#"<p id=status></p><iframe id=seed data-krr-local-frame></iframe><script>
const status = document.getElementById('status');
document.getElementById('seed').addEventListener('load', () => {
  status.textContent += 'seed|';
  const frame = document.createElement('iframe');
  frame.setAttribute('data-krr-local-frame', '');
  frame.addEventListener('load', () => { status.textContent += 'dynamic|'; });
  document.body.appendChild(frame);
});
window.addEventListener('load', () => { status.textContent += 'window|'; });
</script>"#,
    )?;

    assert!(output.contains(">seed|dynamic|window|</p>"), "{output}");
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
fn dispatches_custom_property_handlers_through_the_visible_expando() -> TestResult {
    let output = render(
        r#"<p id=status></p><div id=target></div><script>
const status = document.getElementById('status');
const target = document.getElementById('target');
target.oncustom = () => { status.textContent = 'custom-property'; };
target.dispatchEvent(new Event('custom'));
</script>"#,
    )?;

    assert!(output.contains(">custom-property</p>"), "{output}");
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
fn routes_body_onload_through_the_window_load_lifecycle() -> TestResult {
    let output = render(
        r#"<body data-order="" onload="this.setAttribute('data-order', `${this.getAttribute('data-order')}body|`)"><p id=status></p><img src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==" onload="document.body.setAttribute('data-order', `${document.body.getAttribute('data-order')}resource|`)"><script>
window.onload = () => { document.getElementById('status').textContent = `${document.body.getAttribute('data-order')}window|`; };
</script></body>"#,
    )?;

    assert!(output.contains(">resource|window|</p>"), "{output}");
    Ok(())
}

#[test]
fn dispatches_scriptless_body_onload() -> TestResult {
    let output = render(
        r#"<body onload="document.getElementById('status').textContent = 'body-load'"><p id=status>waiting</p></body>"#,
    )?;

    assert!(output.contains(">body-load</p>"), "{output}");
    Ok(())
}

#[test]
fn body_onload_overrides_a_head_script_in_parser_order() -> TestResult {
    let output = render(
        r#"<head><script>window.onload = () => { document.getElementById('status').textContent = 'head'; };</script></head><body onload="document.getElementById('status').textContent = 'body'"><p id=status></p></body>"#,
    )?;

    assert!(output.contains(">body</p>"), "{output}");
    assert!(!output.contains(">head</p>"), "{output}");
    Ok(())
}

#[test]
fn body_script_overrides_body_onload_after_the_body_start_tag() -> TestResult {
    let output = render(
        r#"<head><script>window.onload = () => { document.getElementById('status').textContent = 'head'; };</script></head><body onload="document.getElementById('status').textContent = 'body'"><p id=status></p><script>window.onload = () => { document.getElementById('status').textContent = 'body-script'; };</script></body>"#,
    )?;

    assert!(output.contains(">body-script</p>"), "{output}");
    assert!(!output.contains(">body</p>"), "{output}");
    assert!(!output.contains(">head</p>"), "{output}");
    Ok(())
}

#[test]
fn body_onload_uses_a_single_window_load_dispatch() -> TestResult {
    let output = render(
        r#"<body onload="document.getElementById('status').textContent = `captures:${captures}:target:${event.target === window}:this:${this === document.body}`"><p id=status></p><script>
let captures = 0;
window.addEventListener('load', () => { captures += 1; }, true);
</script></body>"#,
    )?;

    assert!(
        output.contains(">captures:1:target:true:this:true</p>"),
        "{output}"
    );
    Ok(())
}

#[test]
fn dispatches_scriptless_element_ready_state_handlers() -> TestResult {
    let output = render(
        r#"<p id=status></p><iframe data-krr-local-frame onreadystatechange="document.getElementById('status').textContent += `${document.readyState}|`"></iframe>"#,
    )?;

    assert!(output.contains(">interactive|complete|</p>"), "{output}");
    Ok(())
}

#[test]
fn document_ready_state_handler_failure_does_not_interrupt_element_ready_state_dispatch()
-> TestResult {
    let output = render(
        r#"<p id=status></p><iframe data-krr-local-frame onreadystatechange="document.getElementById('status').textContent += `${document.readyState}|`"></iframe><script>document.onreadystatechange = () => { throw new Error('document ready state failed'); };</script>"#,
    )?;

    assert!(output.contains(">interactive|complete|</p>"), "{output}");
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

fn render(source: &str) -> TestResult<String> {
    HtmlRenderer
        .render(&HtmlRenderInput {
            source: source.to_string(),
        })
        .map(|output| output.content)
        .map_err(|error| format!("HTML runtime must render test fixture: {error}"))
}
