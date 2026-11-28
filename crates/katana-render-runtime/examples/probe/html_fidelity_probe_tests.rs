use super::{ProbeOptions, ProbeResult};
use std::{ffi::OsString, fs, path::Path};

const PATH_ARGUMENT_COUNT: usize = 4;
const SCROLL_FIXTURE: &str = r#"<!doctype html><style>
html,body{margin:0;padding:0}div{height:48px}
</style><div style="background:red"></div><div style="background:lime"></div><div style="background:blue"></div>"#;
const PHYSICAL_WIDTH: u32 = 120;
const PHYSICAL_HEIGHT: u32 = 80;
const COLOR_SAMPLE: u32 = 8;

fn rejected(
    result: ProbeResult<ProbeOptions>,
) -> Result<Box<dyn std::error::Error>, std::io::Error> {
    match result {
        Err(error) => Ok(error),
        Ok(_) => Err(std::io::Error::other("invalid command line was accepted")),
    }
}

#[test]
fn parses_command_arguments_and_preserves_output_configuration() -> ProbeResult<()> {
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("fixture.html");
    let output = directory.path().join("frames");
    fs::write(&input, "<html><body>probe</body></html>")?;

    let options = ProbeOptions::parse(probe_paths(&input, &output).into_iter().chain([
        "--width".into(),
        "640".into(),
        "--height".into(),
        "480".into(),
        "--scale".into(),
        "1.5".into(),
        "--scroll-y".into(),
        "12.5".into(),
        "--scroll-y".into(),
        "900".into(),
    ]))?;

    assert_eq!(options.input, input);
    assert_eq!(options.output, output);
    assert_eq!(
        (options.width, options.height, options.scale),
        (640, 480, 1.5)
    );
    assert_eq!(options.scroll_y, [12.5, 900.0]);
    Ok(())
}

#[test]
fn applies_defaults_when_scroll_positions_are_omitted() -> ProbeResult<()> {
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("fixture.html");
    fs::write(&input, "<html></html>")?;

    let options = ProbeOptions::parse(probe_paths(&input, &directory.path().join("frames")))?;

    assert_eq!(
        (options.width, options.height),
        (super::DEFAULT_WIDTH, super::DEFAULT_HEIGHT)
    );
    assert_eq!(options.scroll_y, [0.0, 600.0, 1_800.0]);
    Ok(())
}

#[test]
fn scaled_probe_captures_requested_css_scroll_positions_and_document_end() -> ProbeResult<()> {
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("fixture.html");
    let output = directory.path().join("frames");
    fs::write(&input, SCROLL_FIXTURE)?;
    let beyond_end = f32::MAX.to_string();
    let positions = ["0", "48", "12", &beyond_end];
    super::run(scroll_arguments(&input, &output, positions))?;

    let colors = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [255, 0, 0, 255],
        [0, 0, 255, 255],
    ];
    for (scroll_y, color) in positions.into_iter().zip(colors) {
        let image =
            image::open(output.join(format!("120x80-scale-2-scroll-{scroll_y}.png")))?.into_rgba8();
        assert_eq!(image.dimensions(), (PHYSICAL_WIDTH, PHYSICAL_HEIGHT));
        assert_eq!(
            image.get_pixel(COLOR_SAMPLE, COLOR_SAMPLE).0,
            color,
            "scroll {scroll_y}"
        );
    }
    Ok(())
}

fn scroll_arguments<'a>(
    input: &Path,
    output: &Path,
    positions: impl IntoIterator<Item = &'a str>,
) -> Vec<OsString> {
    let mut arguments = probe_paths(input, output).to_vec();
    arguments.extend([
        "--width".into(),
        PHYSICAL_WIDTH.to_string().into(),
        "--height".into(),
        PHYSICAL_HEIGHT.to_string().into(),
        "--scale".into(),
        "2".into(),
    ]);
    arguments.extend(
        positions
            .into_iter()
            .flat_map(|scroll| ["--scroll-y".into(), scroll.into()]),
    );
    arguments
}

#[test]
fn rejects_invalid_cli_values_and_missing_input_file() -> ProbeResult<()> {
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("fixture.html");
    fs::write(&input, "<html></html>")?;
    let output = directory.path().join("frames");
    assert_invalid_values(&input, &output)?;
    let missing = directory.path().join("missing.html");
    let error = rejected(ProbeOptions::parse(probe_paths(&missing, &output)))?;
    assert!(error.to_string().contains("input HTML is missing"));
    let error = rejected(ProbeOptions::parse(["--input".into()]))?;
    assert!(error.to_string().contains("requires a value"));
    let error = rejected(ProbeOptions::parse(["--help".into()]))?;
    assert_eq!(error.to_string(), super::USAGE);
    Ok(())
}

fn assert_invalid_values(input: &Path, output: &Path) -> ProbeResult<()> {
    for (flag, value, expected) in [
        ("--width", "0", "width, height, and scale must be positive"),
        ("--height", "0", "width, height, and scale must be positive"),
        (
            "--scale",
            "NaN",
            "width, height, and scale must be positive",
        ),
        (
            "--scroll-y",
            "-1",
            "scroll-y values must be finite and non-negative",
        ),
        (
            "--scroll-y",
            "inf",
            "scroll-y values must be finite and non-negative",
        ),
    ] {
        let arguments = probe_paths(input, output)
            .into_iter()
            .chain([flag.into(), value.into()]);
        let error = rejected(ProbeOptions::parse(arguments))?;
        assert!(error.to_string().contains(expected), "{error}");
    }

    Ok(())
}

fn probe_paths(input: &Path, output: &Path) -> [OsString; PATH_ARGUMENT_COUNT] {
    [
        "--input".into(),
        input.as_os_str().to_owned(),
        "--output".into(),
        output.as_os_str().to_owned(),
    ]
}
