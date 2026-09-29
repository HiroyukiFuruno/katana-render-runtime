use super::{HtmlRenderInput, HtmlRenderer};

type TestResult<T = ()> = Result<T, String>;

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
fn deleting_lifecycle_handler_does_not_restore_the_inline_handler() -> TestResult {
    let output = render(
        r#"<p id=status>unchanged</p><iframe id=frame data-krr-local-frame onload="document.getElementById('status').textContent = 'inline'"></iframe><script>
const frame = document.getElementById('frame');
frame.onload = () => { document.getElementById('status').textContent = 'assigned'; };
delete frame.onload;
</script>"#,
    )?;

    assert!(output.contains(">unchanged</p>"), "{output}");
    assert!(!output.contains(">inline</p>"), "{output}");
    assert!(!output.contains(">assigned</p>"), "{output}");
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
