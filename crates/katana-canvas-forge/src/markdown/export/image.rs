use super::native_document::NativeHtmlDocument;
use super::types::{
    ExportError, ExportFormat, ExportInput, ExportOutput, ExporterTrait, ImageExporter,
};
use crate::markdown::MarkdownError;

impl ImageExporter {
    pub fn is_available() -> bool {
        true
    }

    fn export_file(
        format: &ExportFormat,
        html: &str,
        output: &std::path::Path,
    ) -> Result<(), MarkdownError> {
        let document = NativeHtmlDocument::parse(html)?;
        let image = document.render_image()?;
        if *format == ExportFormat::Jpeg {
            image.save_jpeg(output)
        } else {
            image.save_png(output)
        }
    }
}

static IMAGE_FORMATS: &[ExportFormat] = &[ExportFormat::Png, ExportFormat::Jpeg];

impl ExporterTrait for ImageExporter {
    fn export(&self, input: &ExportInput) -> Result<ExportOutput, ExportError> {
        if !matches!(input.format, ExportFormat::Png | ExportFormat::Jpeg) {
            return Err(ExportError::UnsupportedFormat);
        }
        Self::export_file(&input.format, &input.html_source, &input.output_path)
            .map(|()| ExportOutput {
                output_path: input.output_path.clone(),
                format: input.format.clone(),
            })
            .map_err(|e| ExportError::RenderFailed(e.to_string()))
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        IMAGE_FORMATS
    }
}

#[cfg(test)]
mod tests {
    use super::ImageExporter;
    use crate::markdown::export::{ExportFormat, ExporterTrait};

    #[test]
    fn image_exporter_reports_availability_and_supported_formats() {
        use crate::markdown::export::{ExportConfig, ExportInput};

        assert!(ImageExporter::is_available());
        assert_eq!(
            ImageExporter.supported_formats(),
            &[ExportFormat::Png, ExportFormat::Jpeg]
        );

        let output_path = std::env::temp_dir()
            .join(format!("kcf-image-missing-dir-{}", std::process::id()))
            .join("out.png");
        let input = ExportInput {
            format: ExportFormat::Png,
            html_source: "<html><body>ok</body></html>".to_string(),
            output_path,
            config: ExportConfig::default(),
        };
        assert!(ImageExporter.export(&input).is_err());
    }
}
