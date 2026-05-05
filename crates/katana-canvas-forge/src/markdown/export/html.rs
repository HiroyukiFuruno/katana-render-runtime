use super::html_template::HtmlExportTemplate;
use super::types::{
    ExportError, ExportFormat, ExportInput, ExportOutput, ExporterTrait, HtmlExporter,
};

impl HtmlExporter {
    pub fn assemble_document(css: &str, body: &str) -> String {
        HtmlExportTemplate::assemble_html_document(css, body)
    }

    pub fn css_for_preset(preset: &crate::markdown::color_preset::DiagramColorPreset) -> String {
        HtmlExportTemplate::generate_css(preset)
    }

    pub fn resolve_relative_paths(html: &str, base_dir: &std::path::Path) -> String {
        HtmlExportTemplate::resolve_relative_paths(html, base_dir)
    }
}

static HTML_FORMATS: &[ExportFormat] = &[ExportFormat::Html];

impl ExporterTrait for HtmlExporter {
    fn export(&self, input: &ExportInput) -> Result<ExportOutput, ExportError> {
        if input.format != ExportFormat::Html {
            return Err(ExportError::UnsupportedFormat);
        }
        std::fs::write(&input.output_path, input.html_source.as_bytes())
            .map_err(|e| ExportError::IoError(e.to_string()))?;
        Ok(ExportOutput {
            output_path: input.output_path.clone(),
            format: ExportFormat::Html,
        })
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        HTML_FORMATS
    }
}

#[cfg(test)]
mod tests {
    use super::HtmlExporter;
    use crate::markdown::color_preset::DiagramColorPreset;

    #[test]
    fn assemble_document_keeps_body_and_css() {
        let html = HtmlExporter::assemble_document("body{color:red}", "<main>ok</main>");
        assert!(html.contains("body{color:red}"));
        assert!(html.contains("<main>ok</main>"));
    }

    #[test]
    fn resolve_relative_paths_converts_local_image_src() {
        let html = HtmlExporter::resolve_relative_paths(
            r#"<img src="images/a.png"><img src="https://example.com/b.png">"#,
            std::path::Path::new("/tmp/doc"),
        );
        assert!(html.contains(r#"src="file:///tmp/doc/images/a.png""#));
        assert!(html.contains(r#"src="https://example.com/b.png""#));
    }

    #[test]
    fn css_for_preset_uses_theme_colors() {
        let css = HtmlExporter::css_for_preset(DiagramColorPreset::dark());
        assert!(css.contains("#E0E0E0"));
        assert!(css.contains("font-family"));
    }

    #[test]
    fn html_exporter_reports_supported_format() {
        use crate::markdown::export::{ExportConfig, ExportFormat, ExportInput, ExporterTrait};

        assert_eq!(HtmlExporter.supported_formats(), &[ExportFormat::Html]);

        let output_path = std::env::temp_dir()
            .join(format!("kcf-html-missing-dir-{}", std::process::id()))
            .join("out.html");
        let input = ExportInput {
            format: ExportFormat::Html,
            html_source: "<html></html>".to_string(),
            output_path,
            config: ExportConfig::default(),
        };
        assert!(HtmlExporter.export(&input).is_err());
    }
}
