use super::PdfExporter;
use crate::markdown::export::{ExportConfig, ExportFormat, ExportInput, ExporterTrait};

#[test]
fn pdf_exporter_reports_availability_and_supported_format() {
    assert!(PdfExporter::is_available());
    assert_eq!(PdfExporter.supported_formats(), &[ExportFormat::Pdf]);

    let output_path = std::env::temp_dir()
        .join(format!("kcf-pdf-missing-dir-{}", std::process::id()))
        .join("out.pdf");
    let input = ExportInput {
        format: ExportFormat::Pdf,
        html_source: "<html><body>ok</body></html>".to_string(),
        output_path,
        config: ExportConfig::default(),
    };
    assert!(PdfExporter.export(&input).is_err());
}
