use crate::file_ops::FileOps;
use crate::system::ProcessService;
use anyhow::Context;
use katana_canvas_forge::exporter::{
    ExportConfig, ExportFormat, ExportInput, ExporterTrait, HtmlExporter, ImageExporter,
    PdfExporter,
};
use std::path::{Path, PathBuf};

const DEBUG_OUTPUT_DIR: &str = "/tmp";
const DEBUG_OUTPUT_COUNT: usize = 4;

pub(crate) trait ExportOutputOpener {
    fn open(&mut self, path: &Path) -> anyhow::Result<()>;
}

pub(crate) struct ExportDebugCommand<O: ExportOutputOpener = MacOsOutputOpener> {
    input: PathBuf,
    opener: O,
}

impl ExportDebugCommand<MacOsOutputOpener> {
    pub(crate) fn new(input: PathBuf) -> Self {
        Self {
            input,
            opener: MacOsOutputOpener::new(),
        }
    }
}

impl<O: ExportOutputOpener> ExportDebugCommand<O> {
    pub(crate) fn run(mut self) -> anyhow::Result<()> {
        let html_source = FileOps::read_to_string(&self.input)?;
        let paths = ExportDebugOutputPaths::new();
        ExportDebugExporter::write_all(&html_source, &paths)?;
        self.open_all(&paths)
    }

    fn open_all(&mut self, paths: &ExportDebugOutputPaths) -> anyhow::Result<()> {
        for path in paths.paths() {
            self.opener.open(path)?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn with_opener(input: PathBuf, opener: O) -> Self {
        Self { input, opener }
    }
}

pub(crate) struct MacOsOutputOpener {
    command_name: &'static str,
}

impl MacOsOutputOpener {
    fn new() -> Self {
        Self {
            command_name: "open",
        }
    }

    #[cfg(test)]
    fn with_command(command_name: &'static str) -> Self {
        Self { command_name }
    }
}

impl ExportOutputOpener for MacOsOutputOpener {
    fn open(&mut self, path: &Path) -> anyhow::Result<()> {
        let status = ProcessService::create_command(self.command_name)
            .arg(path)
            .status()
            .with_context(|| format!("failed to run macOS open for {}", path.display()))?;
        MacOsOpenStatus::new(status).ensure(path)
    }
}

struct MacOsOpenStatus {
    status: std::process::ExitStatus,
}

impl MacOsOpenStatus {
    fn new(status: std::process::ExitStatus) -> Self {
        Self { status }
    }

    fn ensure(&self, path: &Path) -> anyhow::Result<()> {
        if self.status.success() {
            return Ok(());
        }
        anyhow::bail!("macOS open failed for {}", path.display());
    }
}

struct ExportDebugOutputPaths {
    html: PathBuf,
    pdf: PathBuf,
    png: PathBuf,
    jpeg: PathBuf,
}

impl ExportDebugOutputPaths {
    fn new() -> Self {
        let base_name = format!("kcf-export-debug-{}", std::process::id());
        Self {
            html: Self::debug_path(&base_name, "html"),
            pdf: Self::debug_path(&base_name, "pdf"),
            png: Self::debug_path(&base_name, "png"),
            jpeg: Self::debug_path(&base_name, "jpg"),
        }
    }

    fn paths(&self) -> [&Path; DEBUG_OUTPUT_COUNT] {
        [
            self.html.as_path(),
            self.pdf.as_path(),
            self.png.as_path(),
            self.jpeg.as_path(),
        ]
    }

    fn debug_path(base_name: &str, extension: &str) -> PathBuf {
        PathBuf::from(DEBUG_OUTPUT_DIR).join(format!("{base_name}.{extension}"))
    }
}

struct ExportDebugExporter;

impl ExportDebugExporter {
    fn write_all(html_source: &str, paths: &ExportDebugOutputPaths) -> anyhow::Result<()> {
        Self::write(HtmlExporter, ExportFormat::Html, html_source, &paths.html)?;
        Self::write(PdfExporter, ExportFormat::Pdf, html_source, &paths.pdf)?;
        Self::write(ImageExporter, ExportFormat::Png, html_source, &paths.png)?;
        Self::write(ImageExporter, ExportFormat::Jpeg, html_source, &paths.jpeg)
    }

    fn write(
        exporter: impl ExporterTrait,
        format: ExportFormat,
        html_source: &str,
        output_path: &Path,
    ) -> anyhow::Result<()> {
        let input = ExportInput {
            format,
            html_source: html_source.to_string(),
            output_path: output_path.to_path_buf(),
            config: ExportConfig::default(),
        };
        exporter.export(&input)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "export_debug_cmd_tests.rs"]
mod tests;
