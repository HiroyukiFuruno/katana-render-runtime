use super::{HtmlRenderInput, HtmlRenderer};
use base64::Engine as _;

type TestResult<T = ()> = Result<T, String>;

#[test]
fn dispatches_image_errors_when_the_data_url_cannot_be_loaded() -> TestResult {
    let output = render(
        r#"<p id=status></p><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=" onload="document.getElementById('status').textContent += 'load|'" onerror="document.getElementById('status').textContent += 'unexpected|'"><img src="data:image/png;base64,!" onload="document.getElementById('status').textContent += 'invalid-load|'" onerror="document.getElementById('status').textContent += 'invalid-error|'"><img src="data:image/png;base64,AAAA" onload="document.getElementById('status').textContent += 'corrupt-load|'" onerror="document.getElementById('status').textContent += 'corrupt-error|'"><img src="data:text/plain;base64,aGVsbG8=" onload="document.getElementById('status').textContent += 'text-load|'" onerror="document.getElementById('status').textContent += 'text-error|'">"#,
    )?;

    assert!(
        output.contains(">load|invalid-error|corrupt-error|text-error|</p>"),
        "{output}"
    );
    Ok(())
}

#[test]
fn preserves_rejected_image_data_urls_for_interactive_scripts_and_dispatches_errors() -> TestResult
{
    let malformed = "data:image/png;base64,!";
    let unsupported = "data:text/plain;base64,aGVsbG8=";
    let unsupported_image = "data:image/bmp;base64,not-decoded-by-javascript";
    let output = render(&format!(
        r#"<p id=status></p><img id=malformed src="{malformed}" onerror="document.getElementById('status').textContent += 'malformed-error|'" onload="document.getElementById('status').textContent += 'malformed-load|'"><img id=unsupported src="{unsupported}" onerror="document.getElementById('status').textContent += 'unsupported-error|'" onload="document.getElementById('status').textContent += 'unsupported-load|'"><img id=unsupported-image src="{unsupported_image}" onerror="document.getElementById('status').textContent += 'unsupported-image-error|'" onload="document.getElementById('status').textContent += 'unsupported-image-load|'"><script>const status=document.getElementById('status');if(document.getElementById('malformed').getAttribute('src')==='{malformed}')status.textContent+='malformed-source|';if(document.getElementById('unsupported').getAttribute('src')==='{unsupported}')status.textContent+='unsupported-source|';if(document.getElementById('unsupported-image').getAttribute('src')==='{unsupported_image}')status.textContent+='unsupported-image-source|';</script>"#
    ))?;

    assert!(
        output.contains(
            ">malformed-source|unsupported-source|unsupported-image-source|malformed-error|unsupported-error|unsupported-image-error|</p>"
        ),
        "{output}"
    );
    Ok(())
}

#[test]
fn dispatches_error_for_truncated_png_after_a_valid_ihdr() -> TestResult {
    let output = render(
        r#"<p id=status></p><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwC" onload="document.getElementById('status').textContent = 'load'" onerror="document.getElementById('status').textContent = 'error'">"#,
    )?;

    assert!(output.contains(">error</p>"), "{output}");
    Ok(())
}

#[test]
fn dispatches_error_for_crc_valid_png_with_invalid_zlib_stream() -> TestResult {
    let output = render(
        r#"<p id=status></p><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAABklEQVR4nAAAAADDcVx9AAAAAElFTkSuQmCC" onload="document.getElementById('status').textContent = 'load'" onerror="document.getElementById('status').textContent = 'error'">"#,
    )?;

    assert!(output.contains(">error</p>"), "{output}");
    Ok(())
}

#[test]
fn does_not_dispatch_image_events_without_a_source_attribute() -> TestResult {
    let output = render(
        r#"<p id=status></p><img onload="document.getElementById('status').textContent += 'placeholder-load|'" onerror="document.getElementById('status').textContent += 'placeholder-error|'"><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=" onload="document.getElementById('status').textContent += 'loaded|'" onerror="document.getElementById('status').textContent += 'unexpected|'">"#,
    )?;

    assert!(output.contains(r#"id="status">loaded|</p>"#), "{output}");
    assert!(!output.contains(r#"id="status">placeholder-"#), "{output}");
    Ok(())
}

#[test]
fn dispatches_load_for_large_valid_svg_data_urls() -> TestResult {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg">{}</svg>"#,
        " ".repeat(256 * 1024)
    );
    let payload = base64::engine::general_purpose::STANDARD.encode(svg);
    let output = render(&format!(
        r#"<p id=status></p><img src="data:image/svg+xml;base64,{payload}" onload="document.getElementById('status').textContent = 'load'" onerror="document.getElementById('status').textContent = 'error'">"#
    ))?;

    assert!(output.contains(">load</p>"), "{output}");
    Ok(())
}

#[test]
fn dispatches_load_for_percent_encoded_image_data_urls() -> TestResult {
    let png = percent_encoded_data_url(
        "image/png",
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
    )?;
    let gif = percent_encoded_data_url(
        "image/gif",
        "R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==",
    )?;
    let jpeg = percent_encoded_data_url(
        "image/jpeg",
        "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/wAALCAAIAAgBAREA/8QAFAABAAAAAAAAAAAAAAAAAAAAAf/EABsQAAIBBQAAAAAAAAAAAAAAABITYyUzQ4KT/9oACAEBAAA/AC9Izcy6ETpCdmdUf//Z",
    )?;
    let webp = percent_encoded_data_url(
        "image/webp",
        "UklGRjwAAABXRUJQVlA4IDAAAADQAQCdASoBAAEAAgA0JaACdLoB+AADsAD+8MQL/yC5YXXI1/8gP+QH/ID/+PIAAAA=",
    )?;
    let svg = "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%201%201%22%3E%3C%2Fsvg%3E";
    let output = render(&format!(
        r#"<p id=status></p><img src="{png}" onload="document.getElementById('status').textContent += 'png|'"><img src="{gif}" onload="document.getElementById('status').textContent += 'gif|'"><img src="{jpeg}" onload="document.getElementById('status').textContent += 'jpeg|'"><img src="{webp}" onload="document.getElementById('status').textContent += 'webp|'"><img src="{svg}" onload="document.getElementById('status').textContent += 'svg|'">"#,
    ))?;

    assert!(output.contains(">png|gif|jpeg|webp|svg|</p>"), "{output}");
    Ok(())
}

#[test]
fn dispatches_load_for_percent_encoded_base64_image_payloads() -> TestResult {
    let source = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII%3D";
    let output = render(&format!(
        r#"<p id=status></p><img src="{source}" onload="document.getElementById('status').textContent = 'load'" onerror="document.getElementById('status').textContent = 'error'">"#
    ))?;

    assert!(output.contains(">load</p>"), "{output}");
    Ok(())
}

#[test]
fn dispatches_error_for_webp_with_incomplete_coded_frame_data() -> TestResult {
    let output = render(
        r#"<p id=status></p><img src="data:image/webp;base64,UklGRhYAAABXRUJQVlA4IAoAAAAAAACdASoBAAEA" onload="document.getElementById('status').textContent = 'load'" onerror="document.getElementById('status').textContent = 'error'">"#,
    )?;

    assert!(output.contains(">error</p>"), "{output}");
    assert!(!output.contains(">load</p>"), "{output}");
    Ok(())
}

#[test]
fn dispatches_load_for_nonzero_ac_baseline_jpeg() -> TestResult {
    let output = render(
        r#"<p id=status></p><img src="data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/wAALCAAIAAgBAREA/8QAFAABAAAAAAAAAAAAAAAAAAAAAf/EABsQAAIBBQAAAAAAAAAAAAAAABITYyUzQ4KT/9oACAEBAAA/AC9Izcy6ETpCdmdUf//Z" onload="document.getElementById('status').textContent = 'load'" onerror="document.getElementById('status').textContent = 'error'">"#,
    )?;

    assert!(output.contains(">load</p>"), "{output}");
    Ok(())
}

#[test]
fn dispatches_load_for_baseline_jpeg_with_restart_markers() -> TestResult {
    let output = render(
        r#"<p id=status></p><img src="data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/2wBDAQkJCQwLDBgNDRgyIRwhMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjL/wAARCAAIACADASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/90ABAAB/9oADAMBAAIRAxEAPwDi6KKK+ZP3E//Q4uiiivmT9xP/2Q==" onload="document.getElementById('status').textContent = 'load'" onerror="document.getElementById('status').textContent = 'error'">"#,
    )?;

    assert!(output.contains(">load</p>"), "{output}");
    Ok(())
}

#[test]
fn dispatches_image_events_from_jpeg_structure_validation() -> TestResult {
    let output = render(
        r#"<p id=status></p><img src="data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAYEBQYFBAYGBQYHBwYIChAKCgkJChQODwwQFxQYGBcUFhYaHSUfGhsjHBYWICwgIyYnKSopGR8tMC0oMCUoKSj/wAALCAABAAEBAREA/8QAFAABAAAAAAAAAAAAAAAAAAAACP/EABQQAQAAAAAAAAAAAAAAAAAAAAD/2gAIAQEAAD8AKj//2Q==" onload="document.getElementById('status').textContent += 'valid-load|'" onerror="document.getElementById('status').textContent += 'valid-error|'"><img src="data:image/jpeg;base64,/9j/2Q==" onload="document.getElementById('status').textContent += 'corrupt-load|'" onerror="document.getElementById('status').textContent += 'corrupt-error|'"><img src="data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAYEBQYFBAYGBQYHBwYIChAKCgkJChQODwwQFxQYGBcUFhYaHSUfGhsjHBYWICwgIyYnKSopGR8tMC0oMCUoKSj/wAALCAABAAEBAREA/8QAFAABAAAAAAAAAAAAAAAAAAAACP/EABQQAQAAAAAAAAAAAAAAAAAAAAD/2gAIAQEAAD8AKn//2Q==" onload="document.getElementById('status').textContent += 'entropy-load|'" onerror="document.getElementById('status').textContent += 'entropy-error|'">"#,
    )?;

    assert!(
        output.contains(">valid-load|corrupt-error|entropy-load|</p>"),
        "{output}"
    );
    Ok(())
}

#[test]
fn dispatches_error_for_gif_without_a_logical_screen_descriptor() -> TestResult {
    let output = render(
        r#"<p id=status></p><img src="data:image/gif;base64,R0lGODlhOw==" onload="document.getElementById('status').textContent = 'load'" onerror="document.getElementById('status').textContent = 'error'">"#,
    )?;

    assert!(output.contains(">error</p>"), "{output}");
    assert!(!output.contains(">load</p>"), "{output}");
    Ok(())
}

#[test]
fn image_lifecycle_listener_failures_do_not_stop_later_images_or_window_load() -> TestResult {
    let image = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==";
    let output = render(&format!(
        r#"<p id=status></p><img src="{image}" onload="throw new Error('first image failed')"><img src="data:image/png;base64,!" onerror="throw new Error('second image failed')"><img src="{image}" onload="document.getElementById('status').textContent += 'later-image|'"><script>window.addEventListener('load', () => {{ document.getElementById('status').textContent += 'window-load|'; }});</script>"#,
    ))?;

    assert!(output.contains(">later-image|window-load|</p>"), "{output}");
    Ok(())
}

#[test]
fn records_the_final_source_after_an_image_handler_replaces_it() -> TestResult {
    let first = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==";
    let replacement = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
    let output = render(&format!(
        r#"<p id=status></p><img id=image src="{first}" onload="this.dataset.count = String(Number(this.dataset.count || 0) + 1); if (!this.dataset.swapped) {{ this.dataset.swapped = '1'; this.src = '{replacement}'; }}"><script>window.onload = () => {{ document.getElementById('status').textContent = document.getElementById('image').dataset.count; }};</script>"#,
    ))?;

    assert!(output.contains(">2</p>"), "{output}");
    Ok(())
}

#[test]
fn dispatches_the_eighth_synchronous_image_replacement_before_window_load() -> TestResult {
    let image = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==";
    let output = render(&format!(
        r#"<p id=status></p><img id=image src="{image}" onload="this.dataset.count = String(Number(this.dataset.count || 0) + 1); if (Number(this.dataset.count) < 9) this.src = '{image}#' + this.dataset.count; else document.getElementById('status').textContent = 'source8';"><script>window.addEventListener('load', () => {{ document.getElementById('status').textContent += '|window'; }});</script>"#,
    ))?;

    assert!(output.contains(">source8|window</p>"), "{output}");
    Ok(())
}

#[test]
fn lifecycle_scheduler_uses_intrinsic_checkpoint_after_page_promise_patch() -> TestResult {
    let image = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==";
    let output = render(&format!(
        r#"<p id=status></p><iframe data-krr-local-frame onload="document.getElementById('status').textContent += 'frame|' "></iframe><img src="{image}" onload="document.getElementById('status').textContent += 'image|' "><script>Promise.resolve = () => {{ throw new Error('patched'); }}; document.addEventListener('readystatechange', () => {{ if (document.readyState === 'complete') document.getElementById('status').textContent += 'complete|'; }}); window.addEventListener('load', () => {{ document.getElementById('status').textContent += 'window|'; }});</script>"#,
    ))?;

    assert!(
        output.contains(">frame|image|complete|window|</p>"),
        "{output}"
    );
    Ok(())
}

fn percent_encoded_data_url(media_type: &str, base64_payload: &str) -> TestResult<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_payload)
        .map_err(|error| format!("fixture must be valid base64: {error}"))?;
    let payload = bytes
        .iter()
        .map(|byte| format!("%{byte:02X}"))
        .collect::<String>();
    Ok(format!("data:{media_type},{payload}"))
}

fn render(html: &str) -> TestResult<String> {
    HtmlRenderer
        .render(&HtmlRenderInput {
            source: html.to_string(),
        })
        .map(|output| output.content)
        .map_err(|error| format!("HTML runtime must render: {error}"))
}
