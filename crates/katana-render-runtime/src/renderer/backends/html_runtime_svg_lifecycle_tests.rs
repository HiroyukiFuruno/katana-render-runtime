use super::{HtmlRenderInput, HtmlRenderer};
use base64::Engine as _;

type TestResult<T = ()> = Result<T, String>;

#[test]
fn dispatches_load_for_svg_data_urls_with_legal_xml_prologs() -> TestResult {
    let variants = [
        "\u{feff}<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        "\u{feff}<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        "<!-- before root --><svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        "<!DOCTYPE svg [<!ELEMENT svg ANY>]><svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        "\u{feff}<?xml version=\"1.0\"?>\n<!-- comment -->\n<!DOCTYPE svg><svg xmlns=\"http://www.w3.org/2000/svg\"/>",
    ];
    let images = variants
        .iter()
        .enumerate()
        .map(|(index, svg)| {
            let payload = base64::engine::general_purpose::STANDARD.encode(svg);
            format!(
                r#"<img src="data:image/svg+xml;base64,{payload}" onload="document.getElementById('status').textContent += '{index}|'" onerror="document.getElementById('status').textContent += 'error-{index}|'">"#
            )
        })
        .collect::<String>();
    let output = render(&format!(r#"<p id=status></p>{images}"#))?;

    assert!(output.contains(">0|1|2|3|4|</p>"), "{output}");
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
