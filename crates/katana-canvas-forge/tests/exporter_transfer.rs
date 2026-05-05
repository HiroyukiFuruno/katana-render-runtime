use katana_canvas_forge::exporter::{
    ExportConfig, ExportError, ExportFormat, ExportInput, ExporterTrait, HtmlExporter,
    ImageExporter, PdfExporter,
};

#[test]
fn html_exporter_writes_html_source_to_requested_path() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        std::env::temp_dir().join(format!("kcf-html-export-{}.html", std::process::id()));
    let input = ExportInput {
        format: ExportFormat::Html,
        html_source: "<!doctype html><html><body>ok</body></html>".to_string(),
        output_path: output_path.clone(),
        config: ExportConfig::default(),
    };

    let output = HtmlExporter.export(&input)?;
    let written = std::fs::read_to_string(&output.output_path)?;
    std::fs::remove_file(&output_path)?;

    assert_eq!(output.format, ExportFormat::Html);
    assert_eq!(written, input.html_source);
    Ok(())
}

#[test]
fn native_exporters_write_pdf_png_and_jpeg() -> Result<(), Box<dyn std::error::Error>> {
    let long_words = "word ".repeat(90);
    let html_source = native_export_html(&long_words);
    let pdf_path = output_path("native", "pdf");
    let png_path = output_path("native", "png");
    let jpeg_path = output_path("native", "jpg");

    PdfExporter.export(&export_input(ExportFormat::Pdf, &html_source, &pdf_path))?;
    ImageExporter.export(&export_input(ExportFormat::Png, &html_source, &png_path))?;
    ImageExporter.export(&export_input(ExportFormat::Jpeg, &html_source, &jpeg_path))?;

    assert!(std::fs::metadata(&pdf_path)?.len() > 0);
    assert!(std::fs::metadata(&png_path)?.len() > 0);
    assert!(std::fs::metadata(&jpeg_path)?.len() > 0);
    remove_outputs([pdf_path, png_path, jpeg_path])?;
    Ok(())
}

fn native_export_html(long_words: &str) -> String {
    r##"
        <!doctype html>
        <html>
          <head>
            <style>
              body { color: #222222; background-color: #ffffff; }
            </style>
          </head>
          <body>
            <h1>Title</h1>
            <h2>Section</h2>
            <h3>Subsection</h3>
            <h4>Detail</h4>
            <h5>Minor</h5>
            <h6>Small</h6>
            <h1></h1>
            <p>body <strong>bold</strong> <em>italic</em></p>
            <p>__LONG_WORDS__</p>
            <blockquote>quote</blockquote>
            <ul><li>first</li><li>second</li></ul>
            <table>
              <thead><tr><th>name</th><th>value</th></tr></thead>
              <tbody><tr><td>alpha</td><td>1</td></tr></tbody>
            </table>
            <pre><code class="language-rust">fn main() {}</code></pre>
            <svg viewBox="0 0 20 10"><rect width="20" height="10" fill="#333333"/></svg>
          </body>
        </html>
    "##
    .replace("__LONG_WORDS__", long_words)
}

#[test]
fn exporters_reject_unsupported_formats() {
    let html_input = export_input(
        ExportFormat::Pdf,
        "<!doctype html><html></html>",
        &output_path("unsupported-html", "html"),
    );
    let pdf_input = export_input(
        ExportFormat::Png,
        "<!doctype html><html></html>",
        &output_path("unsupported-pdf", "pdf"),
    );
    let image_input = export_input(
        ExportFormat::Html,
        "<!doctype html><html></html>",
        &output_path("unsupported-image", "png"),
    );

    assert!(matches!(
        HtmlExporter.export(&html_input),
        Err(ExportError::UnsupportedFormat)
    ));
    assert!(matches!(
        PdfExporter.export(&pdf_input),
        Err(ExportError::UnsupportedFormat)
    ));
    assert!(matches!(
        ImageExporter.export(&image_input),
        Err(ExportError::UnsupportedFormat)
    ));
}

fn output_path(label: &str, extension: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "kcf-export-{}-{}.{}",
        label,
        std::process::id(),
        extension
    ))
}

fn export_input(
    format: ExportFormat,
    html_source: &str,
    output_path: &std::path::Path,
) -> ExportInput {
    ExportInput {
        format,
        html_source: html_source.to_string(),
        output_path: output_path.to_path_buf(),
        config: ExportConfig::default(),
    }
}

fn remove_outputs<const N: usize>(
    paths: [std::path::PathBuf; N],
) -> Result<(), Box<dyn std::error::Error>> {
    for path in paths {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}
