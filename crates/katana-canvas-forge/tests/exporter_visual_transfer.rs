use katana_canvas_forge::exporter::{
    ExportConfig, ExportFormat, ExportInput, ExporterTrait, ImageExporter, PdfExporter,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn native_export_preserves_html_body_background() -> TestResult {
    let png_path = output_path("dark-background", "png");
    let jpeg_path = output_path("dark-background", "jpg");
    let pdf_path = output_path("dark-background", "pdf");

    ImageExporter.export(&export_input(
        ExportFormat::Png,
        dark_export_html(),
        &png_path,
    ))?;
    ImageExporter.export(&export_input(
        ExportFormat::Jpeg,
        dark_export_html(),
        &jpeg_path,
    ))?;
    PdfExporter.export(&export_input(
        ExportFormat::Pdf,
        dark_export_html(),
        &pdf_path,
    ))?;

    let png = image::open(&png_path)?.to_rgba8();
    let jpeg_image = image::open(&jpeg_path)?.to_rgba8();
    let pdf_jpeg = first_pdf_stream(&std::fs::read(&pdf_path)?)?;
    let pdf_image = image::load_from_memory(&pdf_jpeg)?.to_rgba8();
    assert_dark_pixel(png.get_pixel(8, 8).0);
    assert_dark_pixel(jpeg_image.get_pixel(8, 8).0);
    assert_dark_pixel(pdf_image.get_pixel(8, 8).0);
    remove_outputs([png_path, jpeg_path, pdf_path])?;
    Ok(())
}

#[test]
fn native_export_preserves_selector_list_body_background() -> TestResult {
    let png_path = output_path("selector-list-background", "png");
    let jpeg_path = output_path("selector-list-background", "jpg");
    let pdf_path = output_path("selector-list-background", "pdf");

    ImageExporter.export(&export_input(
        ExportFormat::Png,
        selector_list_export_html(),
        &png_path,
    ))?;
    ImageExporter.export(&export_input(
        ExportFormat::Jpeg,
        selector_list_export_html(),
        &jpeg_path,
    ))?;
    PdfExporter.export(&export_input(
        ExportFormat::Pdf,
        selector_list_export_html(),
        &pdf_path,
    ))?;

    let png = image::open(&png_path)?.to_rgba8();
    let jpeg = image::open(&jpeg_path)?.to_rgba8();
    let pdf = first_pdf_stream(&std::fs::read(&pdf_path)?)?;
    let pdf_image = image::load_from_memory(&pdf)?.to_rgba8();
    assert_dark_pixel(png.get_pixel(8, 8).0);
    assert_dark_pixel(jpeg.get_pixel(8, 8).0);
    assert_dark_pixel(pdf_image.get_pixel(8, 8).0);
    remove_outputs([png_path, jpeg_path, pdf_path])?;
    Ok(())
}

#[test]
fn image_export_normalizes_percent_width_svg_before_embedding() -> TestResult {
    let png_path = output_path("percent-width-svg", "png");

    ImageExporter.export(&export_input(
        ExportFormat::Png,
        percent_width_svg_html(),
        &png_path,
    ))?;

    let image = image::open(&png_path)?.to_rgba8();
    assert_dark_pixel(image.get_pixel(850, 90).0);
    remove_outputs([png_path])?;
    Ok(())
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

fn remove_outputs<const N: usize>(paths: [std::path::PathBuf; N]) -> TestResult {
    for path in paths {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn first_pdf_stream(pdf: &[u8]) -> TestResult<Vec<u8>> {
    let start_marker = b"stream\n";
    let end_marker = b"\nendstream";
    let Some(start_position) = pdf
        .windows(start_marker.len())
        .position(|it| it == start_marker)
    else {
        return Err("PDF stream start not found".into());
    };
    let start = start_position + start_marker.len();
    let Some(end_position) = pdf[start..]
        .windows(end_marker.len())
        .position(|it| it == end_marker)
    else {
        return Err("PDF stream end not found".into());
    };
    let end = end_position + start;
    Ok(pdf[start..end].to_vec())
}

fn assert_dark_pixel(pixel: [u8; 4]) {
    assert!(
        pixel[0] < 60 && pixel[1] < 60 && pixel[2] < 60,
        "native export must keep the HTML body background; got rgba({},{},{},{})",
        pixel[0],
        pixel[1],
        pixel[2],
        pixel[3]
    );
}

fn dark_export_html() -> &'static str {
    r##"<!DOCTYPE html>
<html>
<head>
<style>
body { font-family: Arial, sans-serif; background-color: #1e1e1e; color: #E0E0E0; }
</style>
</head>
<body>
<h1>Dark Export</h1>
<div class="katana-diagram mermaid">
<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80" viewBox="0 0 120 80">
<rect x="8" y="8" width="104" height="64" fill="#2D2D2D" stroke="#AAAAAA"/>
<text x="24" y="46" fill="#E0E0E0">Diagram</text>
</svg>
</div>
</body>
</html>"##
}

fn percent_width_svg_html() -> &'static str {
    r##"<!DOCTYPE html>
<html>
<head>
<style>
body { background-color: #1e1e1e; color: #E0E0E0; }
</style>
</head>
<body>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" viewBox="0 0 120 80">
<rect x="0" y="0" width="120" height="80" fill="#ff0000"/>
</svg>
</body>
</html>"##
}

fn selector_list_export_html() -> &'static str {
    r##"<!DOCTYPE html>
<html>
<head>
<style>
html, body { background: #1e1e1e; color: #E0E0E0; }
</style>
</head>
<body>
<h1>Selector List Export</h1>
<p>CSS must apply to native PDF and image exports.</p>
</body>
</html>"##
}
