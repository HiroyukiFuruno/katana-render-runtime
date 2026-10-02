use super::{HtmlRenderInput, HtmlRenderer};

type TestResult<T = ()> = Result<T, String>;

#[test]
fn dispatches_element_ready_state_property_handlers_and_listeners() -> TestResult {
    let output = render(
        r#"<p id=status></p><iframe id=property-handler data-krr-local-frame></iframe><iframe id=listener data-krr-local-frame></iframe><script>
const status = document.getElementById('status');
const propertyHandler = document.getElementById('property-handler');
const listener = document.getElementById('listener');
propertyHandler.onreadystatechange = (event) => {
  status.textContent += `property:${document.readyState}:${event.target === propertyHandler}|`;
};
listener.addEventListener('readystatechange', (event) => {
  status.textContent += `listener:${document.readyState}:${event.target === listener}|`;
});
</script>"#,
    )?;

    assert!(
        output.contains(
            ">property:interactive:true|listener:interactive:true|property:complete:true|listener:complete:true|</p>"
        ),
        "{output}"
    );
    Ok(())
}

#[test]
fn ready_state_handler_failures_do_not_interrupt_later_elements_or_window_load() -> TestResult {
    let output = render(
        r#"<p id=status></p><iframe data-krr-local-frame onreadystatechange="throw new Error('first element failed')"></iframe><iframe data-krr-local-frame onreadystatechange="document.getElementById('status').textContent += `frame:${document.readyState}|`"></iframe><img onreadystatechange="document.getElementById('status').textContent += `image:${document.readyState}|`"><script>window.addEventListener('load', () => { document.getElementById('status').textContent += 'window-load|'; });</script>"#,
    )?;

    assert!(
        output.contains(
            ">frame:interactive|image:interactive|frame:complete|image:complete|window-load|</p>"
        ),
        "{output}"
    );
    Ok(())
}

#[test]
fn deleting_document_ready_state_handler_preserves_it_on_its_prototype() -> TestResult {
    let output = render(
        r#"<p id=status></p><script>
const status = document.getElementById('status');
const handler = () => {
  const documentChainPreserved = Object.getPrototypeOf(Object.getPrototypeOf(document)) === Object.prototype;
  const ordinary = {};
  status.textContent += `${document.readyState}:${document.onreadystatechange === handler}:${documentChainPreserved}:${!('onreadystatechange' in ordinary)}:${!Object.prototype.hasOwnProperty.call(Object.prototype, 'onreadystatechange')}|`;
};
document.onreadystatechange = handler;
delete document.onreadystatechange;
</script>"#,
    )?;

    assert!(
        output.contains(">interactive:true:true:true:true|complete:true:true:true:true|</p>"),
        "{output}"
    );
    Ok(())
}

#[test]
fn deleting_a_document_ready_state_shadow_restores_the_assigned_handler() -> TestResult {
    let output = render(
        r#"<p id=status></p><script>
const status = document.getElementById('status');
const handler = () => { status.textContent += `${document.readyState}|`; };
document.onreadystatechange = handler;
Object.defineProperty(document, 'onreadystatechange', { configurable: true, value: () => { status.textContent += 'shadow|'; } });
delete document.onreadystatechange;
</script>"#,
    )?;

    assert!(output.contains(">interactive|complete|</p>"), "{output}");
    assert!(!output.contains("shadow|"), "{output}");
    Ok(())
}

#[test]
fn deleting_a_cleared_document_ready_state_shadow_does_not_restore_the_handler() -> TestResult {
    let output = render(
        r#"<p id=status>unchanged</p><script>
const status = document.getElementById('status');
document.onreadystatechange = () => { status.textContent = 'assigned'; };
document.onreadystatechange = null;
Object.defineProperty(document, 'onreadystatechange', { configurable: true, value: () => { status.textContent = 'shadow'; } });
delete document.onreadystatechange;
</script>"#,
    )?;

    assert!(output.contains(">unchanged</p>"), "{output}");
    assert!(!output.contains(">assigned</p>"), "{output}");
    assert!(!output.contains(">shadow</p>"), "{output}");
    Ok(())
}

#[test]
fn document_ready_state_shadow_without_an_assigned_handler_is_not_dispatched() -> TestResult {
    let output = render(
        r#"<p id=status></p><script>
const status = document.getElementById('status');
Object.defineProperty(document, 'onreadystatechange', { configurable: true, value: () => { status.textContent = 'shadow'; } });
</script>"#,
    )?;

    assert!(output.contains("<p id=\"status\"></p>"), "{output}");
    assert!(!output.contains("shadow"), "{output}");
    Ok(())
}

fn render(html: &str) -> TestResult<String> {
    HtmlRenderer
        .render(&HtmlRenderInput {
            source: html.to_string(),
        })
        .map(|output| output.content)
        .map_err(|error| format!("HTML runtime must render: {error}"))
}
