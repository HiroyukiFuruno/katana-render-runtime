use crate::commands::ExportFormatArg;
use crate::file_ops::FileOps;
use katana_canvas_forge::exporter::{
    ExportConfig, ExportFormat, ExportInput, ExporterTrait, HtmlExporter, ImageExporter,
    PdfExporter,
};
use std::path::PathBuf;

pub(crate) struct ExportCommand {
    format: ExportFormatArg,
    input: PathBuf,
    output: PathBuf,
}

impl ExportCommand {
    pub(crate) fn new(format: ExportFormatArg, input: PathBuf, output: PathBuf) -> Self {
        Self {
            format,
            input,
            output,
        }
    }

    pub(crate) fn run(self) -> anyhow::Result<()> {
        let html_source = FileOps::read_to_string(&self.input)?;
        let exporter = self.exporter();
        let input = ExportInputFactory::create(self.format, html_source, self.output);
        exporter.export(&input)?;
        Ok(())
    }

    fn exporter(&self) -> Box<dyn ExporterTrait> {
        match self.format {
            ExportFormatArg::Html => Box::new(HtmlExporter),
            ExportFormatArg::Pdf => Box::new(PdfExporter),
            ExportFormatArg::Png | ExportFormatArg::Jpeg => Box::new(ImageExporter),
        }
    }
}

struct ExportInputFactory;

impl ExportInputFactory {
    fn create(format: ExportFormatArg, html_source: String, output_path: PathBuf) -> ExportInput {
        ExportInput {
            format: Self::format(format),
            html_source,
            output_path,
            config: ExportConfig::default(),
        }
    }

    fn format(format: ExportFormatArg) -> ExportFormat {
        match format {
            ExportFormatArg::Html => ExportFormat::Html,
            ExportFormatArg::Pdf => ExportFormat::Pdf,
            ExportFormatArg::Png => ExportFormat::Png,
            ExportFormatArg::Jpeg => ExportFormat::Jpeg,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ExportCommand, ExportInputFactory};
    use crate::commands::ExportFormatArg;
    use katana_canvas_forge::exporter::ExportFormat;

    #[test]
    fn export_input_factory_maps_jpeg_format() {
        let input = ExportInputFactory::create(
            ExportFormatArg::Jpeg,
            "<html></html>".to_string(),
            std::path::PathBuf::from("out.jpg"),
        );
        assert_eq!(input.format, ExportFormat::Jpeg);
        assert_eq!(input.output_path, std::path::PathBuf::from("out.jpg"));
    }

    #[test]
    fn run_exports_html_file() -> Result<(), Box<dyn std::error::Error>> {
        let input = std::env::temp_dir().join(format!("kcf-cli-in-{}.html", std::process::id()));
        let output = std::env::temp_dir().join(format!("kcf-cli-out-{}.html", std::process::id()));

        std::fs::write(&input, "<!doctype html><html><body>ok</body></html>")?;
        ExportCommand::new(ExportFormatArg::Html, input.clone(), output.clone()).run()?;

        assert_eq!(
            std::fs::read_to_string(&output)?,
            "<!doctype html><html><body>ok</body></html>"
        );

        std::fs::remove_file(input)?;
        std::fs::remove_file(output)?;
        Ok(())
    }
}
