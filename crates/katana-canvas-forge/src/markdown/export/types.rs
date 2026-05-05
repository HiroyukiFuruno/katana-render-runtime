pub struct HtmlExporter;
pub struct ImageExporter;
pub struct PdfExporter;

#[derive(Clone, Default, Debug, PartialEq)]
pub enum PaperSize {
    A4,
    #[default]
    Letter,
    Legal,
    Custom {
        width_mm: u32,
        height_mm: u32,
    },
}

const DEFAULT_EXPORT_MARGIN_MM: f32 = 12.7;

#[derive(Clone, Debug, PartialEq)]
pub struct ExportConfig {
    pub paper_size: PaperSize,
    pub margin_mm: f32,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            paper_size: PaperSize::default(),
            margin_mm: DEFAULT_EXPORT_MARGIN_MM,
        }
    }
}

/// Output format for file-level export (HTML is the intermediate; Pdf/Png/Jpeg are file targets).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Html,
    Pdf,
    Png,
    Jpeg,
}

/// Input to `ExporterTrait::export`.
///
/// `html_source` is the already-rendered HTML string.  Callers are responsible
/// for converting Markdown → HTML (e.g. via `HtmlExporter::export_html`) before
/// passing it here.
#[derive(Clone, Debug)]
pub struct ExportInput {
    pub format: ExportFormat,
    pub html_source: String,
    pub output_path: std::path::PathBuf,
    pub config: ExportConfig,
}

impl Default for ExportInput {
    fn default() -> Self {
        Self {
            format: ExportFormat::Png,
            html_source: String::new(),
            output_path: std::path::PathBuf::new(),
            config: ExportConfig::default(),
        }
    }
}

/// Successful result of `ExporterTrait::export`.
#[derive(Debug)]
pub struct ExportOutput {
    pub output_path: std::path::PathBuf,
    pub format: ExportFormat,
}

/// Error returned by `ExporterTrait::export`.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("render failed: {0}")]
    RenderFailed(String),
    #[error("unsupported format")]
    UnsupportedFormat,
}

/// Neutral export interface.  `PdfExporter` and `ImageExporter` implement this.
/// At kcf intake the KatanA-internal impl can be swapped for the external one
/// without touching call sites in `katana-ui`.
pub trait ExporterTrait: Send + Sync {
    fn export(&self, input: &ExportInput) -> Result<ExportOutput, ExportError>;
    fn supported_formats(&self) -> &[ExportFormat];
    /// Returns `true` when the underlying runtime is available on this machine.
    fn is_available(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{ExportConfig, ExportFormat, ExportInput, ExporterTrait, HtmlExporter, PaperSize};

    #[test]
    fn defaults_match_png_letter_export_contract() {
        let input = ExportInput::default();
        assert_eq!(input.format, ExportFormat::Png);
        assert!(input.html_source.is_empty());
        assert!(input.output_path.as_os_str().is_empty());
        assert_eq!(input.config.paper_size, PaperSize::Letter);
        assert_eq!(ExportConfig::default().margin_mm, 12.7);
    }

    #[test]
    fn custom_paper_sizes_and_trait_availability_are_explicit() {
        let custom = PaperSize::Custom {
            width_mm: 210,
            height_mm: 297,
        };

        assert_eq!(
            custom,
            PaperSize::Custom {
                width_mm: 210,
                height_mm: 297
            }
        );
        assert!(HtmlExporter.is_available());
        assert_ne!(PaperSize::A4, PaperSize::Legal);
    }
}
