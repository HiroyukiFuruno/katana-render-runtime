use super::{HtmlRenderInput, HtmlRenderer};

type TestResult<T = ()> = Result<T, String>;

#[test]
fn dispatches_image_events_from_the_shared_data_url_decoder() -> TestResult {
    let output = render(
        r#"<p id=status></p><img src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAf8AOw==" onload="document.getElementById('status').textContent += 'gif-load|'" onerror="document.getElementById('status').textContent += 'gif-error|'"><img src="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'><text>é</text></svg>" onload="document.getElementById('status').textContent += 'svg-load|'" onerror="document.getElementById('status').textContent += 'svg-error|'">"#,
    )?;

    assert!(output.contains(">gif-error|svg-load|</p>"), "{output}");
    Ok(())
}

#[test]
fn rejects_indexed_png_without_palette() -> TestResult {
    let output = render(
        r#"<p id=status></p><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABAQMAAAAl21bKAAAACklEQVR4nGNgAAAAAgABSK+kcQAAAABJRU5ErkJggg==" onload="document.getElementById('status').textContent = 'load'" onerror="document.getElementById('status').textContent = 'error'">"#,
    )?;

    assert!(output.contains(">error</p>"), "{output}");
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
