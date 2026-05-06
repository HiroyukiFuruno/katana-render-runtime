use super::{ExportDebugCommand, ExportOutputOpener, MacOsOpenStatus, MacOsOutputOpener};
use std::cell::RefCell;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn run_writes_four_formats_to_tmp_and_opens_them() -> TestResult {
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

#[test]
fn macos_open_status_reports_failure() -> TestResult {
    let path = PathBuf::from("/tmp/kcf-export-debug-missing.html");
    assert!(
        MacOsOpenStatus::new(std::process::ExitStatus::from_raw(0))
            .ensure(&path)
            .is_ok()
    );
    let error = MacOsOpenStatus::new(std::process::ExitStatus::from_raw(1))
        .ensure(&path)
        .err()
        .ok_or("expected open failure")?;
    assert!(error.to_string().contains("macOS open failed"));
    Ok(())
}

#[test]
fn macos_output_opener_reports_missing_command() -> TestResult {
    let path = PathBuf::from("/tmp/kcf-export-debug-missing.html");
    let mut opener = MacOsOutputOpener::with_command("__kcf_missing_open_command__");
    let error = opener.open(&path).err().ok_or("expected command failure")?;

    assert!(error.to_string().contains("failed to run macOS open"));
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
