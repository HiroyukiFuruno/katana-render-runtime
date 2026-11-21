use super::{AppResult, paths::PreviewPaths, run};
use std::{fs, path::Path};

const WIDTH: u32 = 960;
const HEIGHT: u32 = 720;
const CAPTURES: [&str; 4] = [
    "preview.png",
    "preview-accordion.png",
    "preview-button.png",
    "preview-typed.png",
];

#[test]
fn preview_writes_distinct_interaction_captures_and_self_contained_report() -> AppResult<()> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let output = directory.path();
    run_preview(&PreviewPaths::default_input_path(), output)?;
    assert_captures(output)?;
    let report =
        fs::read_to_string(output.join("index.html")).map_err(|error| error.to_string())?;
    assert_eq!(
        report.matches("data:image/png;base64,").count(),
        CAPTURES.len()
    );
    for label in [
        "Initial render",
        "Accordion opened by click",
        "Button click updated DOM",
        "Text input delivered",
    ] {
        assert!(
            report.contains(label),
            "report capture label missing: {label}"
        );
    }
    assert!(report.contains("linked-page.html"));
    Ok(())
}

#[test]
fn preview_rejects_missing_input_before_creating_outputs() -> AppResult<()> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let output = directory.path();
    let result = run_preview(&output.join("missing.html"), output);
    assert!(result.is_err_and(|error| error.contains("input HTML file is missing")));
    assert!(!output.join("preview.png").exists());
    assert!(!output.join("index.html").exists());
    Ok(())
}

fn run_preview(input: &Path, output: &Path) -> AppResult<()> {
    let paths = [
        input.to_path_buf(),
        output.join("preview.png"),
        output.join("index.html"),
    ];
    let mut arguments = Vec::new();
    for (flag, path) in ["--input", "--out", "--report"].into_iter().zip(paths) {
        arguments.push(flag.to_string());
        arguments.push(
            path.into_os_string()
                .into_string()
                .map_err(|_| "preview path must be UTF-8")?,
        );
    }
    arguments.extend([
        "--width".into(),
        WIDTH.to_string(),
        "--height".into(),
        HEIGHT.to_string(),
        "--scale".into(),
        "1".into(),
    ]);
    run(arguments)
}

fn assert_captures(output: &Path) -> AppResult<()> {
    let mut previous = None;
    for capture in CAPTURES {
        let frame = image::open(output.join(capture)).map_err(|error| error.to_string())?;
        assert_eq!((frame.width(), frame.height()), (WIDTH, HEIGHT));
        let rgba = frame.into_rgba8();
        assert!(rgba.pixels().any(|pixel| pixel[3] != 0));
        if let Some(previous) = previous {
            assert!(rgba != previous, "{capture} pixels did not change");
        }
        previous = Some(rgba);
    }
    Ok(())
}
