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

fn render(html: &str) -> TestResult<String> {
    HtmlRenderer
        .render(&HtmlRenderInput {
            source: html.to_string(),
        })
        .map(|output| output.content)
        .map_err(|error| format!("HTML runtime must render: {error}"))
}
