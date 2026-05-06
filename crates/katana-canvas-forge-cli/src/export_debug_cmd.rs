use crate::file_ops::FileOps;
use anyhow::Context;
use katana_canvas_forge::exporter::{
    ExportConfig, ExportFormat, ExportInput, ExporterTrait, HtmlExporter, ImageExporter,
    PdfExporter,
};
use std::path::{Path, PathBuf};

const DEBUG_OUTPUT_DIR: &str = "/tmp";

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
            opener: MacOsOutputOpener,
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

pub(crate) struct MacOsOutputOpener;

impl ExportOutputOpener for MacOsOutputOpener {
    fn open(&mut self, path: &Path) -> anyhow::Result<()> {
        let status = std::process::Command::new("open")
            .arg(path)
            .status()
            .with_context(|| format!("failed to run macOS open for {}", path.display()))?;
        if status.success() {
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

    fn paths(&self) -> [&Path; 4] {
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
mod tests {
    use super::{ExportDebugCommand, ExportOutputOpener};
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    #[test]
    fn run_writes_four_formats_to_tmp_and_opens_them() -> Result<(), Box<dyn std::error::Error>> {
        let input = PathBuf::from(format!(
            "/tmp/kcf-export-debug-input-{}.html",
            std::process::id()
        ));
        std::fs::write(&input, debug_html())?;
        let opened = Rc::new(RefCell::new(Vec::new()));
        let opener = RecordingOpener::new(Rc::clone(&opened));

        ExportDebugCommand::with_opener(input.clone(), opener).run()?;

        let paths = opened.borrow().clone();
        assert_eq!(paths.len(), 4);
        assert_extensions(&paths);
        for path in &paths {
            assert!(path.starts_with("/tmp"));
            assert!(path.exists());
            std::fs::remove_file(path)?;
        }
        std::fs::remove_file(input)?;
        Ok(())
    }

    struct RecordingOpener {
        opened: Rc<RefCell<Vec<PathBuf>>>,
    }

    impl RecordingOpener {
        fn new(opened: Rc<RefCell<Vec<PathBuf>>>) -> Self {
            Self { opened }
        }
    }

    impl ExportOutputOpener for RecordingOpener {
        fn open(&mut self, path: &Path) -> anyhow::Result<()> {
            self.opened.borrow_mut().push(path.to_path_buf());
            Ok(())
        }
    }

    fn assert_extensions(paths: &[PathBuf]) {
        let extensions = paths
            .iter()
            .filter_map(|it| it.extension())
            .map(|it| it.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(extensions, ["html", "pdf", "png", "jpg"]);
    }

    fn debug_html() -> &'static str {
        "<!doctype html><html><body><h1>debug</h1></body></html>"
    }
}
